#![no_std]

use pinocchio::{
    account::AccountView,
    address::Address,
    entrypoint,
    nostd_panic_handler,
    error::ProgramError,
    ProgramResult,
    cpi::{Seed, Signer},
};

use pinocchio_system::instructions::Transfer;

entrypoint!(process_instruction);
nostd_panic_handler!();

// Program ID using Address::new_from_array as per guidelines
pub const ID: Address = Address::new_from_array([
    0x0f, 0x1e, 0x6b, 0x14, 0x21, 0xc0, 0x4a, 0x07, 0x04, 0x31, 0x26, 0x5c, 0x19, 0xc5, 0xbb, 0xee,
    0x19, 0x92, 0xba, 0xe8, 0xaf, 0xd1, 0xcd, 0x07, 0x8e, 0xf8, 0xaf, 0x70, 0x47, 0xdc, 0x11, 0xf7,
]);

// Updated function signature using new types
fn process_instruction(
    _program_id: &Address,
    accounts: &[AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data.split_first() {
        Some((0, data)) => Deposit::try_from((data, accounts))?.process(),
        Some((1, _)) => Withdraw::try_from(accounts)?.process(),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

pub struct DepositAccounts<'a> {
    pub owner: &'a AccountView,
    pub vault: &'a AccountView,
    pub system_program: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for DepositAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [owner, vault, system_program, _remaining @ ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // Signer check with proper error type
        if !owner.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // Owner check using proper method name
        if !vault.owned_by(&pinocchio_system::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        // Program ID check (prevents arbitrary CPI)
        if system_program.address() != &pinocchio_system::ID {
            return Err(ProgramError::IncorrectProgramId);
        }

        Ok(Self { owner, vault, system_program })
    }
}



pub struct DepositData {
    pub amount: u64,
}

impl TryFrom<&[u8]> for DepositData {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != core::mem::size_of::<u64>() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let amount = u64::from_le_bytes(data.try_into().unwrap());

        // Validate amount is not zero
        if amount == 0 {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(Self { amount })
    }
}

pub struct Deposit<'a> {
    pub accounts: DepositAccounts<'a>,
    pub data: DepositData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountView])> for Deposit<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountView])) -> Result<Self, Self::Error> {
        let accounts = DepositAccounts::try_from(accounts)?;
        let data = DepositData::try_from(data)?;

        Ok(Self { accounts, data })
    }
}

impl<'a> Deposit<'a> {

    pub const DISCRIMINATOR: &'a u8 = &0;
    pub fn process(&self) -> ProgramResult {
        Transfer {
            from: self.accounts.owner,
            to: self.accounts.vault,
            lamports: self.data.amount,
        }
        .invoke()?;

        Ok(())
    }
}


pub struct WithdrawAccounts<'a> {
    pub owner: &'a AccountView,
    pub vault: &'a AccountView,
    pub system_program: &'a AccountView,
    pub bumps: [u8; 1],
}

impl<'a> TryFrom<&'a [AccountView]> for WithdrawAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {

        let [owner, vault, system_program, _remaining@ ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };


        // Signer check with proper error type
        if !owner.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // Owner check
        if !vault.owned_by(&pinocchio_system::ID) {
            return Err(ProgramError::InvalidAccountOwner);
        }

        // Program ID check (prevents arbitrary CPI)
        if system_program.address() != &pinocchio_system::ID {
            return Err(ProgramError::IncorrectProgramId);
        }

        // Check vault has lamports to withdraw
        if vault.lamports() == 0 {
            return Err(ProgramError::InvalidAccountData);
        }

        let bump = 255u8; // You'll need to handle this properly in production

        Ok(Self { owner, vault, system_program, bumps: [bump] })
       
    }
}


pub struct Withdraw<'a> {
    pub accounts: WithdrawAccounts<'a>,
}

impl<'a> TryFrom<&'a [AccountView]> for Withdraw<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let accounts = WithdrawAccounts::try_from(accounts)?;

        Ok(Self { accounts })
    }
}

impl<'a> Withdraw<'a> {

    pub const DISCRIMINATOR: &'a u8 = &1;

    pub fn process(&self) -> ProgramResult {

        // Create PDA signer seeds
        let seeds = [
            Seed::from(b"vault"),
            Seed::from(self.accounts.owner.address().as_ref()),
            Seed::from(&self.accounts.bumps),
        ];

        let signers = [Signer::from(&seeds)];

        // Transfer all lamports from vault to owner
        Transfer {
            from: self.accounts.vault,
            to: self.accounts.owner,
            lamports: self.accounts.vault.lamports(),
        }
        .invoke_signed(&signers)?;

        Ok(())
    }
}
