// Ask opm-common what it makes of a deck, rather than re-deriving it.
//
// C12 spent a long time comparing ResSim's well index and relative permeability against a
// *reimplementation* of Peaceman and of SGOF interpolation. That is a weaker check than it looks:
// it verifies the formula in the plan, not the number the reference actually used. This prints
// opm-common's own values.
//
// It was written for the connection-rate forensics (docs/COMPOSITIONAL_C12_FORENSICS.md §2) and
// settled three rows of that table in one run: the connection transmissibility factor, the
// wellbore radius (which confirms COMPDAT item 9 is a DIAMETER), and the saturation table.
//
// Build:  bash tools/opm_compositional/deck-probe.sh <deck>
#include <opm/input/eclipse/Parser/Parser.hpp>
#include <opm/input/eclipse/Deck/Deck.hpp>
#include <opm/input/eclipse/EclipseState/EclipseState.hpp>
#include <opm/input/eclipse/EclipseState/Grid/EclipseGrid.hpp>
#include <opm/input/eclipse/EclipseState/Tables/TableManager.hpp>
#include <opm/input/eclipse/EclipseState/Tables/SgofTable.hpp>
#include <opm/input/eclipse/EclipseState/Tables/TableColumn.hpp>
#include <opm/input/eclipse/Schedule/Schedule.hpp>
#include <opm/input/eclipse/Schedule/Well/Well.hpp>
#include <opm/input/eclipse/Schedule/Well/Connection.hpp>
#include <opm/input/eclipse/Schedule/Well/WellConnections.hpp>
#include <opm/input/eclipse/Python/Python.hpp>
#include <cstdlib>
#include <iomanip>
#include <iostream>
#include <string>
#include <vector>

namespace {

// A connection transmissibility factor is in SI (m^3). ResSim carries well indices in
// m^3*cP/(day*bar), which is what the METRIC Darcy constant produces. The conversion is
// 1e3 (Pa.s per cP) * 1e5 (Pa per bar) * 86400 (s per day).
constexpr double SI_CF_TO_RESSIM_WELL_INDEX = 1.0e3 * 1.0e5 * 86400.0;

void report_wells(const Opm::Schedule& schedule)
{
    const std::size_t last = schedule.size() - 1;
    for (const auto& name : schedule.wellNames(last)) {
        const auto& well = schedule.getWell(name, last);
        std::cout << "well " << name << "  ref depth " << well.getRefDepth() << " m\n";
        for (const auto& c : well.getConnections()) {
            std::cout << "  (" << c.getI() + 1 << "," << c.getJ() + 1 << "," << c.getK() + 1 << ")"
                      << "  CF = " << c.CF() << " m3"
                      << "  -> well index " << c.CF() * SI_CF_TO_RESSIM_WELL_INDEX
                      << " m3.cP/(day.bar)\n"
                      << "      Kh = " << c.Kh() << "  rw = " << c.rw() << "  r0 = " << c.r0()
                      << "  skin = " << c.skinFactor() << "  depth = " << c.depth()
                      << "  dir = " << Opm::Connection::Direction2String(c.dir()) << "\n";
        }
    }
}

void report_sgof(const Opm::EclipseState& state, const std::vector<double>& at)
{
    const auto& tables = state.getTableManager().getSgofTables();
    if (tables.empty()) {
        std::cout << "no SGOF table\n";
        return;
    }
    const auto& t = tables.getTable<Opm::SgofTable>(0);
    const auto& sg = t.getSgColumn();
    const auto& krg = t.getKrgColumn();
    const auto& kro = t.getKrogColumn();
    std::cout << "SGOF: " << t.numRows() << " rows, Sg from " << sg[0] << " to "
              << sg[t.numRows() - 1] << "\n";
    for (const double s : at) {
        std::size_t j = 0;
        while (j + 2 < t.numRows() && sg[j + 1] < s) {
            ++j;
        }
        const double f = (s - sg[j]) / (sg[j + 1] - sg[j]);
        std::cout << "  at Sg = " << s << ":  krg = " << krg[j] + f * (krg[j + 1] - krg[j])
                  << "  kro = " << kro[j] + f * (kro[j + 1] - kro[j]) << "\n";
    }
}

}  // namespace

int main(int argc, char** argv)
{
    if (argc < 2) {
        std::cerr << "usage: deck_probe DECK [Sg ...]\n";
        return 2;
    }
    std::vector<double> at;
    for (int i = 2; i < argc; ++i) {
        at.push_back(std::atof(argv[i]));
    }

    Opm::Parser parser;
    const auto deck = parser.parseFile(argv[1]);
    Opm::EclipseState state(deck);
    auto python = std::make_shared<Opm::Python>();
    Opm::Schedule schedule(deck, state, python);

    const auto& grid = state.getInputGrid();
    std::cout << std::setprecision(12);
    std::cout << "grid " << grid.getNX() << "x" << grid.getNY() << "x" << grid.getNZ() << "\n";
    report_wells(schedule);
    if (!at.empty()) {
        report_sgof(state, at);
    }
    return 0;
}
