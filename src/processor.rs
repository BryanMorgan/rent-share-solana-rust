use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke,
    program_error::ProgramError,
    program_pack::IsInitialized,
    pubkey::Pubkey,
    system_instruction,
    sysvar::{rent::Rent, Sysvar},
};

use crate::{
    error::RentShareError,
    instruction::RentShareInstruction,
    state::{AgreementStatus, RentShareAccount},
};

pub struct Processor;
impl Processor {
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = RentShareInstruction::unpack(instruction_data)?;

        match instruction {
            RentShareInstruction::InitializeRentContract {
                payee_pubkey,
                payer_pubkey,
                deposit,
                rent_amount,
                duration,
                duration_unit,
            } => Self::initialize_rent_contract(
                accounts,
                program_id,
                payee_pubkey,
                payer_pubkey,
                deposit,
                rent_amount,
                duration,
                duration_unit,
            ),
            RentShareInstruction::PayRent { rent_amount } => {
                Self::pay_rent(accounts, program_id, rent_amount)
            }
            RentShareInstruction::TerminateEarly {} => Self::terminate_early(accounts, program_id),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn initialize_rent_contract(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
        payee_pubkey: Pubkey,
        payer_pubkey: Pubkey,
        deposit: u64,
        rent_amount: u64,
        duration: u64,
        duration_unit: u8,
    ) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let rent_agreement_account = next_account_info(accounts_iter)?;
        if rent_agreement_account.owner != program_id {
            msg!("[RentShare] Rent agreement account not owned by this program");
            return Err(ProgramError::IncorrectProgramId);
        }

        let solana_rent = &Rent::from_account_info(next_account_info(accounts_iter)?)?;
        // Make sure this account is rent exemtpt
        if !solana_rent.is_exempt(
            rent_agreement_account.lamports(),
            rent_agreement_account.data_len(),
        ) {
            msg!(
                "[RentShare] Rent agreement account not rent exempt. Balance: {}",
                rent_agreement_account.lamports()
            );
            return Err(ProgramError::AccountNotRentExempt);
        }

        // Initialize the Rent Agreement Account with the initial data
        // Note: the structure of the data state must match the `space` reserved when account created
        let rent_agreement_data =
            RentShareAccount::try_from_slice(&rent_agreement_account.data.borrow());

        if rent_agreement_data.is_err() {
            msg!(
                "[RentShare] Rent agreement account data size incorrect: {}",
                rent_agreement_account.try_data_len()?
            );
            return Err(ProgramError::InvalidAccountData);
        }

        let mut rent_data = rent_agreement_data.unwrap();
        if rent_data.is_initialized() {
            msg!("[RentShare] Rent agreement already initialized");
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        rent_data.status = AgreementStatus::Active as u8;
        rent_data.payee_pubkey = payee_pubkey;
        rent_data.payer_pubkey = payer_pubkey;
        rent_data.rent_amount = rent_amount;
        rent_data.deposit = deposit;
        rent_data.duration = duration;
        rent_data.duration_unit = duration_unit;
        rent_data.remaining_payments = duration;
        rent_data.serialize(&mut &mut rent_agreement_account.data.borrow_mut()[..])?;

        msg!(
            "[RentShare] Initialized rent agreement account: {:?}",
            rent_data
        );

        Ok(())
    }

    fn pay_rent(accounts: &[AccountInfo], program_id: &Pubkey, rent_amount: u64) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let rent_agreement_account = next_account_info(accounts_iter)?;
        if rent_agreement_account.owner != program_id {
            msg!("[RentShare] Rent agreement account is not owned by this program");
            return Err(ProgramError::IncorrectProgramId);
        }

        let payee_account: &AccountInfo = next_account_info(accounts_iter)?;
        let payer_account = next_account_info(accounts_iter)?;
        let system_program_account = next_account_info(accounts_iter)?;

        if !payer_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        if payer_account.lamports() < rent_amount {
            return Err(ProgramError::InsufficientFunds);
        }

        // Transfer to self - do nothing
        if payer_account.key == payee_account.key {
            return Ok(());
        }

        // Initialize the Rent Agreement Account with the initial data
        // Note: the structure of the data state must match the `space` the client used to create the account
        let rent_agreement_data =
            RentShareAccount::try_from_slice(&rent_agreement_account.data.borrow());

        if rent_agreement_data.is_err() {
            msg!(
                "[RentShare] Rent agreement account data size incorrect: {}",
                rent_agreement_account.try_data_len()?
            );
            return Err(ProgramError::InvalidAccountData);
        }

        let mut rent_data = rent_agreement_data.unwrap();
        if !rent_data.is_initialized() {
            msg!("[RentShare] Rent agreement account not initialized");
            return Err(ProgramError::UninitializedAccount);
        }

        // Make sure we pay the same account used during the agreement initialization
        if rent_data.payee_pubkey != *payee_account.key {
            msg!("[RentShare] Payee must match payee key used during agreement initialization");
            return Err(ProgramError::InvalidAccountData);
        }

        msg!(
            "[RentShare] Transfer {} lamports from payer with balance: {}",
            rent_amount,
            payer_account.lamports()
        );

        if rent_data.is_complete() {
            msg!("[RentShare] Rent already paid in full");
            return Err(RentShareError::RentAlreadyPaidInFull.into());
        }

        if rent_data.is_terminated() {
            msg!("[RentShare] Rent agreement already terminated");
            return Err(RentShareError::RentAgreementTerminated.into());
        }

        if rent_data.rent_amount != rent_amount {
            msg!(
                "[RentShare] Rent amount does not match agreement amount: {} vs {}",
                rent_data.rent_amount,
                rent_amount
            );
            return Err(RentShareError::RentPaymentAmountMismatch.into());
        }

        let instruction =
            system_instruction::transfer(payer_account.key, payee_account.key, rent_amount);

        // Invoke the system program to transfer funds
        invoke(
            &instruction,
            &[
                system_program_account.clone(),
                payee_account.clone(),
                payer_account.clone(),
            ],
        )?;

        msg!(
            "[RentShare] Transfer completed. New payer balance: {}",
            payer_account.lamports()
        );

        // Decrement the number of payment
        rent_data.remaining_payments -= 1;
        if rent_data.remaining_payments == 0 {
            rent_data.status = AgreementStatus::Completed as u8;
        }
        rent_data.serialize(&mut &mut rent_agreement_account.data.borrow_mut()[..])?;

        Ok(())
    }

    fn terminate_early(accounts: &[AccountInfo], program_id: &Pubkey) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let rent_agreement_account = next_account_info(accounts_iter)?;
        if rent_agreement_account.owner != program_id {
            msg!("[RentShare] Rent agreement account is not owned by this program");
            return Err(ProgramError::IncorrectProgramId);
        }

        let rent_agreement_data =
            RentShareAccount::try_from_slice(&rent_agreement_account.data.borrow());

        if rent_agreement_data.is_err() {
            msg!(
                "[RentShare] Rent agreement account data size incorrect: {}",
                rent_agreement_account.try_data_len()?
            );
            return Err(ProgramError::InvalidAccountData);
        }

        let mut rent_data = rent_agreement_data.unwrap();
        if !rent_data.is_initialized() {
            msg!("[RentShare] Rent agreement account not initialized");
            return Err(ProgramError::UninitializedAccount);
        }

        if rent_data.is_complete() {
            msg!("[RentShare] Rent already paid in full");
            return Err(RentShareError::RentAlreadyPaidInFull.into());
        }

        if rent_data.is_terminated() {
            msg!("[RentShare] Rent agreement already terminated");
            return Err(RentShareError::RentAgreementTerminated.into());
        }

        rent_data.remaining_payments = 0;
        rent_data.status = AgreementStatus::Terminated as u8;
        rent_data.serialize(&mut &mut rent_agreement_account.data.borrow_mut()[..])?;

        Ok(())
    }
}
