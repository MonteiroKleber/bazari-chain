#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod escrow {
    use ink::prelude::string::String;
    use ink::storage::Mapping;

    /// Status do escrow
    #[derive(Debug, Clone, Copy, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum EscrowStatus {
        /// Aguardando depósito
        Pending,
        /// Valor depositado, aguardando entrega
        Funded,
        /// Entrega confirmada, aguardando liberação
        Delivered,
        /// Liberado para o vendedor
        Released,
        /// Reembolsado para o comprador
        Refunded,
        /// Em disputa
        Disputed,
    }

    /// Dados do escrow
    #[derive(Debug, Clone, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct EscrowData {
        pub buyer: AccountId,
        pub seller: AccountId,
        pub amount: Balance,
        pub status: EscrowStatus,
        pub description: String,
        pub created_at: Timestamp,
        pub deadline: Timestamp,
    }

    /// Eventos
    #[ink(event)]
    pub struct EscrowCreated {
        #[ink(topic)]
        id: u64,
        buyer: AccountId,
        seller: AccountId,
        amount: Balance,
    }

    #[ink(event)]
    pub struct EscrowFunded {
        #[ink(topic)]
        id: u64,
    }

    #[ink(event)]
    pub struct DeliveryConfirmed {
        #[ink(topic)]
        id: u64,
    }

    #[ink(event)]
    pub struct EscrowReleased {
        #[ink(topic)]
        id: u64,
        to: AccountId,
        amount: Balance,
    }

    #[ink(event)]
    pub struct EscrowRefunded {
        #[ink(topic)]
        id: u64,
        to: AccountId,
        amount: Balance,
    }

    #[ink(event)]
    pub struct DisputeOpened {
        #[ink(topic)]
        id: u64,
        opened_by: AccountId,
        reason: String,
    }

    /// Storage
    #[ink(storage)]
    pub struct Escrow {
        /// Contador de escrows
        next_id: u64,
        /// Mapping de escrows
        escrows: Mapping<u64, EscrowData>,
        /// Mediador de disputas
        mediator: AccountId,
        /// Taxa do serviço (em basis points, 100 = 1%)
        fee_bps: u16,
        /// Conta que recebe as taxas
        fee_recipient: AccountId,
    }

    /// Erros
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        EscrowNotFound,
        NotBuyer,
        NotSeller,
        NotMediator,
        InvalidStatus,
        InsufficientDeposit,
        DeadlineExpired,
        TransferFailed,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    impl Escrow {
        /// Cria o contrato de escrow
        #[ink(constructor)]
        pub fn new(mediator: AccountId, fee_bps: u16) -> Self {
            Self {
                next_id: 0,
                escrows: Mapping::new(),
                mediator,
                fee_bps,
                fee_recipient: Self::env().caller(),
            }
        }

        /// Cria novo escrow
        #[ink(message)]
        pub fn create_escrow(
            &mut self,
            seller: AccountId,
            amount: Balance,
            description: String,
            deadline_hours: u64,
        ) -> Result<u64> {
            let buyer = Self::env().caller();
            let now = Self::env().block_timestamp();
            let deadline = now + (deadline_hours * 3600 * 1000); // ms

            let id = self.next_id;
            self.next_id += 1;

            let escrow = EscrowData {
                buyer,
                seller,
                amount,
                status: EscrowStatus::Pending,
                description,
                created_at: now,
                deadline,
            };

            self.escrows.insert(id, &escrow);

            Self::env().emit_event(EscrowCreated {
                id,
                buyer,
                seller,
                amount,
            });

            Ok(id)
        }

        /// Deposita fundos no escrow
        #[ink(message, payable)]
        pub fn fund(&mut self, id: u64) -> Result<()> {
            let mut escrow = self.escrows.get(id).ok_or(Error::EscrowNotFound)?;

            if Self::env().caller() != escrow.buyer {
                return Err(Error::NotBuyer);
            }

            if escrow.status != EscrowStatus::Pending {
                return Err(Error::InvalidStatus);
            }

            let deposited = Self::env().transferred_value();
            if deposited < escrow.amount {
                return Err(Error::InsufficientDeposit);
            }

            escrow.status = EscrowStatus::Funded;
            self.escrows.insert(id, &escrow);

            Self::env().emit_event(EscrowFunded { id });

            Ok(())
        }

        /// Confirma entrega (pelo comprador)
        #[ink(message)]
        pub fn confirm_delivery(&mut self, id: u64) -> Result<()> {
            let mut escrow = self.escrows.get(id).ok_or(Error::EscrowNotFound)?;

            if Self::env().caller() != escrow.buyer {
                return Err(Error::NotBuyer);
            }

            if escrow.status != EscrowStatus::Funded {
                return Err(Error::InvalidStatus);
            }

            escrow.status = EscrowStatus::Delivered;
            self.escrows.insert(id, &escrow);

            Self::env().emit_event(DeliveryConfirmed { id });

            // Auto-release após confirmação
            self.release(id)?;

            Ok(())
        }

        /// Libera fundos para o vendedor
        #[ink(message)]
        pub fn release(&mut self, id: u64) -> Result<()> {
            let mut escrow = self.escrows.get(id).ok_or(Error::EscrowNotFound)?;
            let caller = Self::env().caller();

            // Só buyer ou mediator podem liberar
            if caller != escrow.buyer && caller != self.mediator {
                return Err(Error::NotBuyer);
            }

            if escrow.status != EscrowStatus::Funded
                && escrow.status != EscrowStatus::Delivered
                && escrow.status != EscrowStatus::Disputed
            {
                return Err(Error::InvalidStatus);
            }

            // Calcular taxa
            let fee = (escrow.amount * self.fee_bps as u128) / 10_000;
            let seller_amount = escrow.amount - fee;

            // Transferir para vendedor
            if Self::env().transfer(escrow.seller, seller_amount).is_err() {
                return Err(Error::TransferFailed);
            }

            // Transferir taxa
            if fee > 0 {
                let _ = Self::env().transfer(self.fee_recipient, fee);
            }

            escrow.status = EscrowStatus::Released;
            self.escrows.insert(id, &escrow);

            Self::env().emit_event(EscrowReleased {
                id,
                to: escrow.seller,
                amount: seller_amount,
            });

            Ok(())
        }

        /// Solicita reembolso
        #[ink(message)]
        pub fn refund(&mut self, id: u64) -> Result<()> {
            let mut escrow = self.escrows.get(id).ok_or(Error::EscrowNotFound)?;
            let caller = Self::env().caller();
            let now = Self::env().block_timestamp();

            // Seller pode refundar a qualquer momento
            // Mediator pode refundar em disputa
            // Buyer pode refundar após deadline
            let can_refund = caller == escrow.seller
                || (caller == self.mediator && escrow.status == EscrowStatus::Disputed)
                || (caller == escrow.buyer && now > escrow.deadline);

            if !can_refund {
                return Err(Error::NotSeller);
            }

            if escrow.status != EscrowStatus::Funded
                && escrow.status != EscrowStatus::Disputed
            {
                return Err(Error::InvalidStatus);
            }

            // Transferir de volta para comprador
            if Self::env().transfer(escrow.buyer, escrow.amount).is_err() {
                return Err(Error::TransferFailed);
            }

            escrow.status = EscrowStatus::Refunded;
            self.escrows.insert(id, &escrow);

            Self::env().emit_event(EscrowRefunded {
                id,
                to: escrow.buyer,
                amount: escrow.amount,
            });

            Ok(())
        }

        /// Abre disputa
        #[ink(message)]
        pub fn open_dispute(&mut self, id: u64, reason: String) -> Result<()> {
            let mut escrow = self.escrows.get(id).ok_or(Error::EscrowNotFound)?;
            let caller = Self::env().caller();

            if caller != escrow.buyer && caller != escrow.seller {
                return Err(Error::NotBuyer);
            }

            if escrow.status != EscrowStatus::Funded {
                return Err(Error::InvalidStatus);
            }

            escrow.status = EscrowStatus::Disputed;
            self.escrows.insert(id, &escrow);

            Self::env().emit_event(DisputeOpened {
                id,
                opened_by: caller,
                reason,
            });

            Ok(())
        }

        /// Consulta escrow
        #[ink(message)]
        pub fn get_escrow(&self, id: u64) -> Option<EscrowData> {
            self.escrows.get(id)
        }

        /// Consulta mediador
        #[ink(message)]
        pub fn get_mediator(&self) -> AccountId {
            self.mediator
        }

        /// Atualiza mediador (só owner)
        #[ink(message)]
        pub fn set_mediator(&mut self, new_mediator: AccountId) -> Result<()> {
            if Self::env().caller() != self.fee_recipient {
                return Err(Error::NotMediator);
            }
            self.mediator = new_mediator;
            Ok(())
        }
    }
}
