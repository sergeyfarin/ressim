"""Hand-rolled parser for Eclipse/OPM-Flow-style text summary (.RSM) files.

Validation status: this parser's column-layout assumptions were reverse-
engineered from and validated against **real** `flow 2026.04` RUNSUM/SEPARATE
output (both `wf_bl1d` and `spe1_gas_injection`, including a well-name row),
not guessed from documentation alone — see `tests/fixtures/*.RSM`, which are
trimmed excerpts of actual Flow output, not hand-authored samples. A
different Flow version could still format differently; if a future real run
disagrees, fix this module and its fixtures together, and re-verify with a
fresh real run rather than only patching the fixture.

RSM layout, as actually emitted by `flow 2026.04`:
  - One or more "pages". Each page after the first starts with a bare
    page-number line (e.g. "2").
  - A dashed separator line, a "SUMMARY OF RUN ... at: <date>" title line,
    and another dashed separator — both decorative; neither is the
    header/data boundary.
  - A header block: a mnemonic row (first token "TIME"), a unit row, and up
    to two further optional rows in either order: a **scale-factor row**
    (e.g. "*10**3" under a column whose displayed values must be multiplied
    by that power of ten — discovered 2026-07-17 via a real run where FGIR
    exceeded the display width) and/or a **name row** (well/group scoping,
    e.g. WBHP), present only when at least one vector needs it. The two are
    told apart by content, not position: a cell matching the pattern
    "*10**N" (optionally negative N) is a scale factor; anything else
    non-blank is a name. The header block can
    therefore be 2, 3, or 4 non-blank rows. It can be followed by extra
    blank padding lines before the next separator.
  - The dashed separator immediately after the header block, then
    fixed-width data rows, one per report step. TIME is always the first
    column and is repeated on every page; pages are merged by requiring an
    identical TIME axis across pages.

Column alignment: fields are fixed-width and of *uniform* width across every
column (confirmed empirically: consecutive mnemonic-token start positions
are separated by a constant gap, e.g. 13 characters). Column boundaries are
derived from that gap, anchored at the first (TIME) token's start position —
**not** from dividing the separator's total width evenly across columns,
which fails whenever there is left/right margin that isn't itself a data
column (the real files have exactly this: a 1-character left margin and
substantial unused trailing width on every line). Data rows are sliced with
those same boundaries rather than whitespace-split: a value can fill an
entire field with no leading space against its neighbor (observed with real
FGIR/FGPT magnitudes), and a short value (e.g. a bare integer "1" on a
report day with no fractional component) is right-justified to the *end* of
its field while the mnemonic label above it is left-flush at the field's
start — both fall inside the same [start, start+width) slice, so bounding
by range rather than exact token position handles both.

Finding the header/data separator specifically requires scanning *forward*
from the TIME row, not simply taking the first dashed line in the page: the
decorative separators flanking the title line come first and are not it.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

_TOKEN_RE = re.compile(r"\S+")
_PAGE_MARKER_RE = re.compile(r"^\d+$")
_SCALE_FACTOR_RE = re.compile(r"\*10\*\*(-?\d+)")


@dataclass(frozen=True)
class SummaryVector:
    mnemonic: str
    well_or_group: str | None
    unit: str
    values: list[float]

    @property
    def curve_id(self) -> str:
        return f"{self.mnemonic}:{self.well_or_group}" if self.well_or_group else self.mnemonic


@dataclass(frozen=True)
class SummaryData:
    time_days: list[float]
    vectors: list[SummaryVector]

    def by_curve_id(self) -> dict[str, SummaryVector]:
        return {vector.curve_id: vector for vector in self.vectors}


def find_summary_file(run_dir: Path) -> Path | None:
    """Locate the text summary file OPM Flow writes for a RUNSUM'd deck."""
    for pattern in ("*.RSM", "*.rsm"):
        matches = sorted(run_dir.glob(pattern))
        if matches:
            return matches[0]
    return None


def parse_rsm(text: str) -> SummaryData:
    pages = _split_pages(text.splitlines())
    if not pages:
        raise ValueError("No summary pages found in .RSM text")

    merged_time: list[float] | None = None
    vectors: list[SummaryVector] = []
    for page_lines in pages:
        page_time, page_vectors = _parse_page(page_lines)
        if merged_time is None:
            merged_time = page_time
        elif page_time != merged_time:
            raise ValueError(
                "RSM pages have mismatched TIME axes; cannot merge "
                f"(first page has {len(merged_time)} rows, another has {len(page_time)})"
            )
        vectors.extend(page_vectors)

    return SummaryData(time_days=merged_time or [], vectors=vectors)


def _split_pages(lines: list[str]) -> list[list[str]]:
    pages: list[list[str]] = []
    current: list[str] = []
    for line in lines:
        if _PAGE_MARKER_RE.match(line.strip()) and current:
            pages.append(current)
            current = []
            continue
        current.append(line)
    if current:
        pages.append(current)
    return pages


def _parse_page(lines: list[str]) -> tuple[list[float], list[SummaryVector]]:
    # Anchor on the mnemonic row (first token "TIME") by scanning forward,
    # not on "the first dashed line in the page": the page also has a title
    # line flanked by two decorative separators before the real header
    # block, so taking the first separator found would grab the wrong one.
    mnemonic_idx = next(
        (i for i, line in enumerate(lines) if _first_token_is_time(line)),
        None,
    )
    if mnemonic_idx is None:
        raise ValueError("Could not find a header row starting with TIME in this RSM page")

    sep_idx = next(
        (i for i in range(mnemonic_idx + 1, len(lines)) if _is_separator(lines[i])),
        None,
    )
    if sep_idx is None:
        raise ValueError("Could not find the data separator ('---...') after the TIME header row")

    header_lines = [line for line in lines[mnemonic_idx:sep_idx] if line.strip()]
    if len(header_lines) not in (2, 3, 4):
        raise ValueError(
            f"Expected 2-4 non-blank header rows above the separator, found {len(header_lines)}"
        )

    mnemonic_line = header_lines[0]
    unit_line = header_lines[1]
    extra_lines = header_lines[2:]

    mnemonic_tokens = [(m.group(), m.start()) for m in _TOKEN_RE.finditer(mnemonic_line)]
    num_columns = len(mnemonic_tokens)
    if num_columns < 2:
        raise ValueError("Mnemonic header row needs TIME plus at least one vector")

    gaps = {mnemonic_tokens[i + 1][1] - mnemonic_tokens[i][1] for i in range(num_columns - 1)}
    if len(gaps) != 1:
        raise ValueError(
            f"Mnemonic columns are not uniformly spaced (gaps found: {sorted(gaps)}); "
            "cannot derive fixed-width column boundaries"
        )
    field_width = gaps.pop()
    first_start = mnemonic_tokens[0][1]
    col_starts = [first_start + i * field_width for i in range(num_columns)]

    mnemonics = _extract_columns(mnemonic_line, col_starts)
    units = _extract_columns(unit_line, col_starts)

    # Up to two more header rows, told apart by content rather than
    # position: a scale-factor row ("*10**3") or a well/group name row.
    # Whichever is present, absent columns default to no scale / no name.
    names = [""] * num_columns
    scale_factors = [1.0] * num_columns
    for extra_line in extra_lines:
        cells = _extract_columns(extra_line, col_starts)
        non_blank = [cell for cell in cells if cell]
        if non_blank and all(_SCALE_FACTOR_RE.fullmatch(cell) for cell in non_blank):
            for i, cell in enumerate(cells):
                if cell:
                    scale_factors[i] = _parse_scale_factor(cell)
        else:
            for i, cell in enumerate(cells):
                if cell:
                    names[i] = cell

    if mnemonics[0].upper() != "TIME":
        raise ValueError(f"First RSM column must be TIME, got '{mnemonics[0]}'")

    # Data rows are sliced with the same fixed-width column boundaries as the
    # header, not split on whitespace: large values can fill an entire field
    # with no leading space (e.g. an 11-char-wide column holding
    # "2831680.000"), which would otherwise silently merge with its
    # neighbor under naive `.split()` tokenizing.
    data_rows: list[list[float]] = []
    for line in lines[sep_idx + 1 :]:
        if not line.strip():
            continue
        cells = _extract_columns(line, col_starts)
        try:
            data_rows.append([float(cell) for cell in cells])
        except ValueError:
            # Footer/trailer lines that don't parse as a full row of
            # numbers; skip rather than fail the whole page.
            continue

    if not data_rows:
        raise ValueError("No numeric data rows found under the RSM header/separator")

    time_days = [row[0] for row in data_rows]
    vectors: list[SummaryVector] = []
    for col in range(1, len(col_starts)):
        scale = scale_factors[col]
        vectors.append(
            SummaryVector(
                mnemonic=mnemonics[col],
                well_or_group=names[col] or None,
                unit=units[col],
                values=[row[col] * scale for row in data_rows],
            )
        )

    return time_days, vectors


def _is_separator(line: str) -> bool:
    stripped = line.strip()
    return len(stripped) > 10 and set(stripped) == {"-"}


def _first_token_is_time(line: str) -> bool:
    tokens = _TOKEN_RE.findall(line)
    return bool(tokens) and tokens[0].upper() == "TIME"


def _parse_scale_factor(cell: str) -> float:
    match = _SCALE_FACTOR_RE.fullmatch(cell)
    if not match:
        raise ValueError(f"'{cell}' is not a valid scale-factor cell")
    return 10.0 ** int(match.group(1))


def _extract_columns(line: str, col_starts: list[int]) -> list[str]:
    result = []
    for i, start in enumerate(col_starts):
        end = col_starts[i + 1] if i + 1 < len(col_starts) else None
        result.append(line[start:end].strip() if start < len(line) else "")
    return result
