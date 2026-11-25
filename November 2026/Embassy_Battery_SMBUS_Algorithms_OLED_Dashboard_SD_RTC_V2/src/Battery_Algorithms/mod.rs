#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

pub mod PerCellDiag_and_Diff1RC_FFRLS;

// Expose the relevant structs and constants from the PerCellDiag_and_Diff1RC_FFRLS module
pub use PerCellDiag_and_Diff1RC_FFRLS::
{
    Diff1RcFfrlsOutput,
    PerCellDiagOutput,
    PerCellDiag_and_Diff1RC_Output,
    MAX_PACK_CELLS,
};