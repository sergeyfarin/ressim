"""Point lookup for the #116 M1 continuation (stdlib only, so the tool tests can cover it)."""


def point_for(points, prefix, cell, suffix):
    """The point with this case tag *and* cell.

    The M0 file has one record per flipping cell under each case tag, so a tag-only lookup returns
    the first cell for every cell in a multi-cell case. The cell is part of the key.
    """
    tag = f"{prefix} {suffix}"
    for point in points:
        if point.get("tag") == tag and point.get("cell") == cell:
            return point
    return None
