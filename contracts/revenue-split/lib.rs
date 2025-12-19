#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod revenue_split {
    use ink::prelude::vec::Vec;
    use ink::storage::Mapping;

    /// Participante do split
    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct Participant {
        pub account: AccountId,
        /// Percentual em basis points (100 = 1%, 10000 = 100%)
        pub share_bps: u16,
    }

    /// Eventos
    #[ink(event)]
    pub struct RevenueReceived {
        amount: Balance,
    }

    #[ink(event)]
    pub struct RevenueDistributed {
        total: Balance,
        participants: u32,
    }

    #[ink(event)]
    pub struct Withdrawal {
        #[ink(topic)]
        account: AccountId,
        amount: Balance,
    }

    #[ink(event)]
    pub struct ParticipantAdded {
        #[ink(topic)]
        account: AccountId,
        share_bps: u16,
    }

    #[ink(event)]
    pub struct ParticipantRemoved {
        #[ink(topic)]
        account: AccountId,
    }

    /// Storage
    #[ink(storage)]
    pub struct RevenueSplit {
        /// Owner do contrato
        owner: AccountId,
        /// Lista de participantes
        participants: Vec<Participant>,
        /// Saldo pendente de saque por conta
        pending_withdrawals: Mapping<AccountId, Balance>,
        /// Total já distribuído
        total_distributed: Balance,
    }

    /// Erros
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        NotOwner,
        InvalidShares,
        NoBalance,
        TransferFailed,
        ParticipantExists,
        ParticipantNotFound,
        SharesExceed100Percent,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    impl RevenueSplit {
        /// Cria contrato de divisão de receita
        #[ink(constructor)]
        pub fn new(participants: Vec<Participant>) -> Result<Self> {
            // Validar que soma dos shares = 100%
            let total_shares: u32 = participants
                .iter()
                .map(|p| p.share_bps as u32)
                .sum();

            if total_shares != 10_000 {
                return Err(Error::InvalidShares);
            }

            Ok(Self {
                owner: Self::env().caller(),
                participants,
                pending_withdrawals: Mapping::new(),
                total_distributed: 0,
            })
        }

        /// Recebe e distribui receita automaticamente
        #[ink(message, payable)]
        pub fn receive_revenue(&mut self) {
            let amount = Self::env().transferred_value();

            if amount == 0 {
                return;
            }

            Self::env().emit_event(RevenueReceived { amount });

            // Distribuir para cada participante
            for participant in &self.participants {
                let share = (amount * participant.share_bps as u128) / 10_000;

                let current = self
                    .pending_withdrawals
                    .get(participant.account)
                    .unwrap_or(0);

                self.pending_withdrawals
                    .insert(participant.account, &(current + share));
            }

            self.total_distributed += amount;

            Self::env().emit_event(RevenueDistributed {
                total: amount,
                participants: self.participants.len() as u32,
            });
        }

        /// Saca saldo pendente
        #[ink(message)]
        pub fn withdraw(&mut self) -> Result<Balance> {
            let caller = Self::env().caller();
            let amount = self.pending_withdrawals.get(caller).unwrap_or(0);

            if amount == 0 {
                return Err(Error::NoBalance);
            }

            // Zerar saldo antes de transferir (reentrancy protection)
            self.pending_withdrawals.insert(caller, &0);

            if Self::env().transfer(caller, amount).is_err() {
                // Reverter saldo se falhar
                self.pending_withdrawals.insert(caller, &amount);
                return Err(Error::TransferFailed);
            }

            Self::env().emit_event(Withdrawal {
                account: caller,
                amount,
            });

            Ok(amount)
        }

        /// Consulta saldo pendente de uma conta
        #[ink(message)]
        pub fn pending_balance_of(&self, account: AccountId) -> Balance {
            self.pending_withdrawals.get(account).unwrap_or(0)
        }

        /// Consulta lista de participantes
        #[ink(message)]
        pub fn get_participants(&self) -> Vec<Participant> {
            self.participants.clone()
        }

        /// Consulta share de um participante
        #[ink(message)]
        pub fn share_of(&self, account: AccountId) -> u16 {
            self.participants
                .iter()
                .find(|p| p.account == account)
                .map(|p| p.share_bps)
                .unwrap_or(0)
        }

        /// Consulta total distribuído
        #[ink(message)]
        pub fn get_total_distributed(&self) -> Balance {
            self.total_distributed
        }

        /// Adiciona participante (requer rebalancear shares)
        #[ink(message)]
        pub fn add_participant(
            &mut self,
            account: AccountId,
            share_bps: u16,
        ) -> Result<()> {
            if Self::env().caller() != self.owner {
                return Err(Error::NotOwner);
            }

            // Verificar se já existe
            if self.participants.iter().any(|p| p.account == account) {
                return Err(Error::ParticipantExists);
            }

            // Verificar se não excede 100%
            let current_total: u32 = self
                .participants
                .iter()
                .map(|p| p.share_bps as u32)
                .sum();

            if current_total + share_bps as u32 > 10_000 {
                return Err(Error::SharesExceed100Percent);
            }

            self.participants.push(Participant { account, share_bps });

            Self::env().emit_event(ParticipantAdded { account, share_bps });

            Ok(())
        }

        /// Remove participante
        #[ink(message)]
        pub fn remove_participant(&mut self, account: AccountId) -> Result<()> {
            if Self::env().caller() != self.owner {
                return Err(Error::NotOwner);
            }

            let idx = self
                .participants
                .iter()
                .position(|p| p.account == account)
                .ok_or(Error::ParticipantNotFound)?;

            self.participants.remove(idx);

            Self::env().emit_event(ParticipantRemoved { account });

            Ok(())
        }

        /// Atualiza share de participante
        #[ink(message)]
        pub fn update_share(
            &mut self,
            account: AccountId,
            new_share_bps: u16,
        ) -> Result<()> {
            if Self::env().caller() != self.owner {
                return Err(Error::NotOwner);
            }

            let participant = self
                .participants
                .iter_mut()
                .find(|p| p.account == account)
                .ok_or(Error::ParticipantNotFound)?;

            // Verificar novo total
            let other_shares: u32 = self
                .participants
                .iter()
                .filter(|p| p.account != account)
                .map(|p| p.share_bps as u32)
                .sum();

            if other_shares + new_share_bps as u32 > 10_000 {
                return Err(Error::SharesExceed100Percent);
            }

            participant.share_bps = new_share_bps;

            Ok(())
        }
    }
}
