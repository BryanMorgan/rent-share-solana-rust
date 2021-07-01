use thiserror::Error;

use solana_program::program_error::ProgramError;

#[derive(Error, Debug, Copy, Clone)]
pub enum RentShareError {
    /// Rent already paid in full
    #[error("Rent Already Paid In Full")]
    RentAlreadyPaidInFull = 100,
}

impl From<RentShareError> for ProgramError {
    fn from(e: RentShareError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
