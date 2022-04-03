use solana_program::{program_error::ProgramError, pubkey::Pubkey};
use std::convert::TryInto;

#[derive(Debug)]
pub enum RentShareInstruction {
    /// Initialize the rent contract with the agreed on terms and persist initial state in the agreement account
    ///
    /// Accounts expected:
    /// 0. `[writable]` The Rent Agreement account created to manage state across 2 parties; owned by program id.
    /// 1. `[]` Sysvar Rent Account to validate rent exemption (SYSVAR_RENT_PUBKEY)
    InitializeRentContract {
        payee_pubkey: Pubkey,
        payer_pubkey: Pubkey,
        deposit: u64,
        rent_amount: u64,
        duration: u64,
        duration_unit: u8,
    },

    /// Pay rent from payee to payer
    ///
    /// Accounts expected:
    /// 0. `[writable]` The Rent Agreement account created to manage state across 2 parties; owned by program id.
    /// 1. `[signer]` Payer (Renter) account (keypair)
    /// 2. `[]` Payee (Owner) account (public key)
    /// 3. `[]` System program account
    PayRent { rent_amount: u64 },

    /// Terminate agreement early, violating the terms
    ///
    /// Accounts expected:
    /// 0. `[writable]` The Rent Agreement account created to manage state across 2 parties; owned by program id.
    TerminateEarly {},
}

impl RentShareInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (tag, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;

        Ok(match tag {
            0 => {
                let payee_pubkey: Pubkey = Pubkey::new(&rest[..32]);
                let payer_pubkey: Pubkey = Pubkey::new(&rest[32..64]);
                let deposit: u64 = Self::unpack_u64(rest, 64)?;
                let rent_amount: u64 = Self::unpack_u64(rest, 72)?;
                let duration: u64 = Self::unpack_u64(rest, 80)?;
                let duration_unit: u8 = rest[88];

                Self::InitializeRentContract {
                    payee_pubkey,
                    payer_pubkey,
                    deposit,
                    rent_amount,
                    duration,
                    duration_unit,
                }
            }
            1 => {
                let rent_amount: u64 = Self::unpack_u64(rest, 0)?;
                Self::PayRent { rent_amount }
            }
            2 => Self::TerminateEarly {},
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }

    fn unpack_u64(input: &[u8], start: usize) -> Result<u64, ProgramError> {
        let value = input
            .get(start..8 + start)
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(ProgramError::InvalidInstructionData)?;
        Ok(value)
    }
}
