
#![no_std]
#![no_main]
#![allow(non_snake_case)]
#![allow(unused_attributes)]

imports!();

const NAME:     &[u8]    = b"Binance USD";
const SYMBOL:   &[u8]    = b"BUSD";
const DECIMALS: usize    = 18;

#[elrond_wasm_derive::contract(BUSDCoinImpl)]
pub trait BUSDCoin {

    // STATIC INFO

    #[view]
    fn name(&self) -> &'static [u8] {
        NAME
    }

    #[view]
    fn symbol(&self) -> &'static [u8] {
        SYMBOL
    }

    #[view]
    fn decimals(&self) -> usize {
        DECIMALS
    }

    // CONSTRUCTOR

    /// constructor function
    /// is called immediately after the contract is created
    #[init]
    fn init(&self) {
        // owner will be deploy caller
        let owner = self.get_caller();
        self.set_contract_owner(&owner);
        
        // owner is also the initial supply controller
        self.set_supply_controller(&owner);
        
        self.set_asset_protection_role(None);
        self.set_proposed_owner(None);
    
        // the contract starts paused
        self.set_paused(true);
    }

    // ERC20 LOGIC

    /// Total number of tokens in existence.
    #[view(totalSupply)]
    #[storage_get_mut("total_supply")]
    fn get_mut_total_supply(&self) -> mut_storage!(BigUint);

    fn perform_transfer(&self, sender: Address, recipient: Address, amount: BigUint) -> Result<(), &str> {        
        // check if enough funds & decrease sender balance
        {
            let mut sender_balance = self.get_mut_balance(&sender);
            if &amount > &*sender_balance {
                return Err("insufficient funds");
            }
            
            *sender_balance -= &amount; // saved automatically at the end of scope
        }

        // increase recipient balance
        {
            let mut recipient_balance = self.get_mut_balance(&recipient);
            *recipient_balance += &amount; // saved automatically at the end of scope
        }
    
        // log operation
        self.transfer_event(&sender, &recipient, &amount);

        Ok(())
    }

    /// Transfer token to a specified address from sender.
    /// 
    /// Arguments:
    /// 
    /// * `to` The address to transfer to.
    /// 
    #[endpoint]
    fn transfer(&self, to: Address, amount: BigUint) -> Result<(), &str> {
        if self.is_paused() {
            return Err("paused");
        }
        
        // sender is the caller
        let sender = self.get_caller();

        if self.is_frozen(&sender) || self.is_frozen(&to) {
            return Err("address frozen");
        }

        self.perform_transfer(sender, to, amount)
    }

    /// Gets the balance of the specified address.
    /// 
    /// Arguments:
    /// 
    /// * `address` The address to query the the balance of
    /// 
    #[view(balanceOf)]
    #[storage_get("balance")]
    fn balance_of(&self, address: &Address) -> BigUint;

    #[storage_set("balance")]
    fn set_balance(&self, address: &Address, balance: &BigUint);

    #[storage_get_mut("balance")]
    fn get_mut_balance(&self, address: &Address) -> mut_storage!(BigUint);

    // ERC20 FUNCTIONALITY
 
    /// Use allowance to transfer funds between two accounts.
    /// 
    /// Arguments:
    /// 
    /// * `sender` The address to transfer from.
    /// * `recipient` The address to transfer to.
    /// * `amount` the amount of tokens to be transferred.
    /// 
    #[endpoint(transferFrom)]
    fn transfer_from(&self, sender: Address, recipient: Address, amount: BigUint) -> Result<(), &str> {
        if self.is_paused() {
            return Err("paused");
        }
        
        // get caller
        let caller = self.get_caller();

        if self.is_frozen(&caller) || self.is_frozen(&sender) || self.is_frozen(&recipient) {
            return Err("address frozen");
        }

        // load allowance
        let mut allowance = self.get_mut_allowance(&sender, &caller);

        // amount should not exceed allowance
        if &amount > &*allowance {
            return Err("allowance exceeded");
        }

        // update allowance
        *allowance -= &amount; // saved automatically at the end of scope

        // transfer
        self.perform_transfer(sender, recipient, amount)
    }

    /// Approve the given address to spend the specified amount of tokens on behalf of the sender.
    /// It overwrites any previously existing allowance from sender to beneficiary.
    /// 
    /// Arguments:
    /// 
    /// * `spender` The address that will spend the funds.
    /// * `amount` The amount of tokens to be spent.
    /// 
    #[endpoint]
    fn approve(&self, spender: Address, amount: BigUint) -> Result<(), &str> {
        if self.is_paused() {
            return Err("paused");
        }

        // sender is the caller
        let caller = self.get_caller();

        if self.is_frozen(&caller) || self.is_frozen(&spender) {
            return Err("address frozen");
        }

        // store allowance
        self.set_allowance(&caller, &spender, &amount);
      
        // log operation
        self.approve_event(&caller, &spender, &amount);
        Ok(())
    }

    /// Function to check the amount of tokens that an owner allowed to a spender.
    /// 
    /// Arguments:
    /// 
    /// * `owner` The address that owns the funds.
    /// * `spender` The address that will spend the funds.
    /// 
    #[view(allowance)]
    #[storage_get_mut("allowance")]
    fn get_mut_allowance(&self, owner: &Address, spender: &Address) -> mut_storage!(BigUint);

    #[storage_set("allowance")]
    fn set_allowance(&self, owner: &Address, spender: &Address, allowance: &BigUint);

    // OWNER FUNCTIONALITY

    /// Yields the current contract owner.
    #[view(getContractOwner)]
    #[storage_get("owner")]
    fn get_contract_owner(&self) -> Address;

    #[storage_set("owner")]
    fn set_contract_owner(&self, owner: &Address);

    #[storage_get("prop_owner")]
    fn get_proposed_owner(&self) -> Option<Address>;

    /// Yields the currently proposed new owner, if any.
    #[view(getProposedOwner)]
    fn get_proposed_owner_public(&self) -> OptionalResult<Address> {
        self.get_proposed_owner().into()
    }

    #[storage_set("prop_owner")]
    fn set_proposed_owner(&self, proposed_owner: Option<&Address>);

    /// Allows the current owner to begin transferring control of the contract to a proposedOwner
    /// 
    /// Arguments:
    /// 
    /// * `proposed_owner` The address to transfer ownership to.
    /// 
    #[endpoint(proposeOwner)]
    fn propose_owner(&self, proposed_owner: Address) -> Result<(), &str> {
        let caller = self.get_caller();
        if caller != self.get_contract_owner() {
            return Err("only owner can propose another owner");
        }
        if caller == proposed_owner {
            return Err("current owner cannot propose itself");
        }
        if let Some(previous_proposed_owner) = self.get_proposed_owner() {
            if proposed_owner == previous_proposed_owner {
                return Err("caller already is proposed owner"); 
            }
        }

        self.set_proposed_owner(Some(&proposed_owner));

        // event
        self.ownership_transfer_proposed_event(&caller, &proposed_owner, ());
        Ok(())
    }

    /// Allows the current owner or proposed owner to cancel transferring control of the contract to the proposed owner.
    #[endpoint(disregardProposedOwner)]
    fn disregard_proposed_owner() -> Result<(), &str> {
        match self.get_proposed_owner() {
            None => Err("can only disregard a proposed owner that was previously set"),
            Some(proposed_owner) => {
                let caller = self.get_caller();
                if caller != self.get_contract_owner() && caller != proposed_owner {
                    return Err("only proposedOwner or owner can disregard proposed owner"); 
                }
                self.set_proposed_owner(None);

                self.ownership_transfer_disregarded_event(&proposed_owner, ());
                Ok(())
            }
        }
    }

    /// Allows the proposed owner to complete transferring control of the contract to herself..
    #[endpoint(claimOwnership)]
    fn claim_ownership() -> Result<(), &str> {
        match self.get_proposed_owner() {
            None => Err("no owner proposed"),
            Some(proposed_owner) => {
                let caller = self.get_caller();
                if caller != proposed_owner {
                    return Err("only proposed owner can claim ownership")
                }
                
                let old_owner = self.get_contract_owner();

                // set new owner
                self.set_contract_owner(&proposed_owner);
                // clear proposed owner
                self.set_proposed_owner(None);

                self.ownership_transferred_event(&old_owner, &proposed_owner, ());
                Ok(())  
            }
        }
    }

    /// Reclaim all BUSD at the contract address.
    /// This sends all the BUSD tokens that the address of the contract itself is holding to the owner.
    /// Note: this is not affected by freeze constraints.
    #[endpoint(reclaimBUSD)]
    fn reclaim_busd() -> Result<(), &str> {
        let caller = self.get_caller();
        if caller != self.get_contract_owner() {
            return Err("only owner can reclaim"); 
        }

        // load contract own balance
        let contract_address = self.get_sc_address();
        let mut contract_balance = self.get_mut_balance(&contract_address);

        // increment owner balance
        let mut owner_balance = self.get_mut_balance(&caller);
        *owner_balance += &*contract_balance; // saved automatically at the end of scope
    
        // log operation
        self.transfer_event(&contract_address, &caller, &contract_balance);

        // clear contract own balance
        (*contract_balance) = BigUint::zero();

        Ok(())
    }

    // PAUSABILITY FUNCTIONALITY

    #[view(isPaused)]
    #[storage_get("paused")]
    fn is_paused(&self) -> bool;

    #[storage_set("paused")]
    fn set_paused(&self, paused: bool);

    /// Called by the owner to pause, triggers stopped state
    #[endpoint]
    fn pause(&self) -> Result<(), &str> {
        if self.is_paused() {
            return Err("already paused")
        }
        self.set_paused(true);

        self.pause_event(());
        Ok(())
    }

    /// Called by the owner to unpause, returns to normal state
    #[endpoint]
    fn unpause(&self) -> Result<(), &str> {
        if !self.is_paused() {
            return Err("already unpaused")
        }
        self.set_paused(false);

        self.unpause_event(());
        Ok(())
    }

    // ASSET PROTECTION FUNCTIONALITY

    #[storage_get("ap_role")]
    fn get_asset_protection_role(&self) -> Option<Address>;

    #[storage_set("ap_role")]
    fn set_asset_protection_role(&self, ap_role: Option<&Address>);

    /// Yields the current asset protection role, if any.
    #[view(getAssetProtectionRole)]
    fn get_asset_protection_role_public(&self) -> OptionalResult<Address> {
        self.get_asset_protection_role().into()
    }

    fn caller_is_asset_protection_role(&self) -> bool {
        if let Some(asset_prot_role) = self.get_asset_protection_role() {
            if self.get_caller() == asset_prot_role {
                return true;
            }
        }
        false
    }

    /// Sets a new asset Protection role address.
    /// 
    /// Arguments:
    /// 
    /// * `new_asset_prot_role` The new address allowed to freeze/unfreeze addresses and seize their tokens.
    /// 
    #[endpoint(setAssetProtectionRole)]
    fn set_asset_protection_role_endpoint(&self, new_asset_prot_role: &Address) -> Result<(), &str> {
        let caller = self.get_caller();
        if caller != self.get_contract_owner() && 
           !self.caller_is_asset_protection_role() {
            return Err("only asset protection role or owner can change asset protection role")
        }

        // needed for logging
        let old_asset_protection_role = self
            .get_asset_protection_role()
            .unwrap_or_else(|| Address::zero());

        // change asset protection role
        self.set_asset_protection_role(Some(new_asset_prot_role));

        // log event
        self.asset_protection_role_set_event(
            &old_asset_protection_role,
            new_asset_prot_role,
            ()
        );

        Ok(())
    }

    /// Freezes an address balance, preventing any transfers involving it.
    /// 
    /// Arguments:
    /// 
    /// * `address` The address to freeze.
    /// 
    #[endpoint]
    fn freeze(&self, address: &Address) -> Result<(), &str> {
        if !self.caller_is_asset_protection_role() {
            return Err("only asset protection role can freeze");
        }
        if self.is_frozen(&address) {
            return Err("address already frozen");
        }
        self.set_frozen(&address, true);

        self.address_frozen_event(&address, ());
        Ok(())
    }

    /// Unfreezes an address balance, allowing transfers involving it.
    /// 
    /// Arguments:
    /// 
    /// * `address` The address to unfreeze.
    /// 
    #[endpoint]
    fn unfreeze(&self, address: &Address) -> Result<(), &str> {
        if !self.caller_is_asset_protection_role() {
            return Err("only asset protection role can unfreeze");
        }
        if !self.is_frozen(&address) {
            return Err("address already unfrozen");
        }
        self.set_frozen(&address, false);

        self.address_unfrozen_event(&address, ());
        Ok(())
    }

    /// Wipes the balance of a frozen address, burning the tokens
    /// and setting the approval to zero.
    /// 
    /// Arguments:
    /// 
    /// * `address` The address to wipe.
    /// 
    #[endpoint(wipeFrozenAddress)]
    fn wipe_frozen_address(&self, address: &Address) -> Result<(), &str> {
        if !self.caller_is_asset_protection_role() {
            return Err("only asset protection role can wipe");
        }
        if !self.is_frozen(&address) {
            return Err("address is not frozen");
        }

        // erase balance
        let mut balance_to_wipe = self.get_mut_balance(&address);

        // decrease total supply
        let mut total_supply = self.get_mut_total_supply();
        *total_supply -= &*balance_to_wipe;

        // log operation
        self.frozen_address_wiped_event(&address, ());
        self.supply_decreased_event(&address, &*balance_to_wipe);
        self.transfer_event(&address,  &[0u8; 32].into(), &*balance_to_wipe);

        // erase balance
        *balance_to_wipe = BigUint::zero(); // saved automatically at the end of scope

        Ok(())
    }

    /// Gets whether the address is currently frozen.
    /// 
    /// Arguments:
    /// 
    /// * `address` The address to check if frozen.
    /// 
    #[view(isFrozen)]
    #[storage_get("frozen")]
    fn is_frozen(&self, address: &Address) -> bool;

    #[storage_set("frozen")]
    fn set_frozen(&self, address: &Address, frozen: bool);

    // SUPPLY CONTROL FUNCTIONALITY

    /// Yields the currently proposed new owner, if any.
    #[view(getSupplyController)]
    #[storage_get("supply_c")]
    fn get_supply_controller(&self) -> Address;

    #[storage_set("supply_c")]
    fn set_supply_controller(&self, address: &Address);

    fn caller_is_supply_controller(&self) -> bool {
        return self.get_caller() == self.get_supply_controller()
    }

    /// Sets a new supply controller address.
    /// 
    /// Arguments:
    /// 
    /// * `new_supply_controller` The address allowed to burn/mint tokens to control supply.
    /// 
    #[endpoint(setSupplyController)]
    fn set_supply_controller_endpoint(&self, new_supply_controller: &Address) -> Result<(), &str> {
        let caller = self.get_caller();
        if caller != self.get_contract_owner() && 
           !self.caller_is_supply_controller() {
            return Err("only supply controller or owner can change supply controller")
        }

        // needed for logging
        let old_supply_controller = self.get_supply_controller();

        // change supply controller
        self.set_supply_controller(&new_supply_controller);

        // log event
        self.supply_controller_set_event(
            &old_supply_controller,
            new_supply_controller,
            ()
        );

        Ok(())
    }

    /// Increases the total supply by minting the specified number of tokens to the supply controller account.
    /// 
    /// Arguments:
    /// 
    /// * `amount` The number of tokens to add.
    /// 
    #[endpoint(increaseSupply)]
    fn increase_supply(&self, amount: BigUint) -> Result<(), &str> {
        if !self.caller_is_supply_controller() {
            return Err("only supply controller can increase supply");
        }
        let supply_controller = self.get_caller();

        // increase supply controller balance
        let mut supply_contr_balance = self.get_mut_balance(&supply_controller);
        *supply_contr_balance += &amount; // saved automatically at the end of scope

        // increase total supply
        let mut total_supply = self.get_mut_total_supply();
        *total_supply += &amount; // saved automatically at the end of scope

        // log operation
        self.supply_increased_event(&supply_controller, &amount);
        self.transfer_event(&[0u8; 32].into(), &supply_controller, &amount);

        Ok(())
    }

    /// Decreases the total supply by burning the specified number of tokens from the supply controller account.
    /// 
    /// Arguments:
    /// 
    /// * `amount` The number of tokens to remove.
    /// 
    #[endpoint(decreaseSupply)]
    fn decrease_supply(&self, amount: BigUint) -> Result<(), &str> {
        if !self.caller_is_supply_controller() {
            return Err("only supply controller can decrease supply");
        }
        let supply_controller = self.get_caller();

        // get supply controller balance
        let mut supply_contr_balance = self.balance_of(&supply_controller);

        // check
        if amount > supply_contr_balance {
            return Err("not enough supply to decrease")
        }

        // decrease supply controller balance
        supply_contr_balance -= &amount;
        self.set_balance(&supply_controller, &supply_contr_balance);

        // decrease total supply
        let mut total_supply = self.get_mut_total_supply();
        *total_supply -= &amount; // saved automatically at the end of scope

        // log operation
        self.supply_decreased_event(&supply_controller, &amount);
        self.transfer_event(&supply_controller, &[0u8; 32].into(), &amount);

        Ok(())
    }

    // ERC20 BASIC EVENTS
    
    #[event("0x0000000000000000000000000000000000000000000000000000000000000001")]
    fn transfer_event(&self,
        sender: &Address,
        recipient: &Address,
        amount: &BigUint);

    // ERC20 EVENTS

    #[event("0x0000000000000000000000000000000000000000000000000000000000000002")]
    fn approve_event(&self,
        sender: &Address,
        recipient: &Address,
        amount: &BigUint);

    // OWNABLE EVENTS

    #[event("0x0000000000000000000000000000000000000000000000000000000000000003")]
    fn ownership_transfer_proposed_event(&self, 
        current_owner: &Address,
        proposed_owner: &Address,
        _data: ());

    #[event("0x0000000000000000000000000000000000000000000000000000000000000004")]
    fn ownership_transfer_disregarded_event(&self, 
        old_proposed_owner: &Address,
        _data: ());

    #[event("0x0000000000000000000000000000000000000000000000000000000000000005")]
    fn ownership_transferred_event(&self, 
        old_owner: &Address,
        new_owner: &Address,
        _data: ());
    
    // PAUSABLE EVENTS

    #[event("0x0000000000000000000000000000000000000000000000000000000000000006")]
    fn pause_event(&self, _data: ());

    #[event("0x0000000000000000000000000000000000000000000000000000000000000007")]
    fn unpause_event(&self, _data: ());

    // ASSET PROTECTION EVENTS

    #[event("0x0000000000000000000000000000000000000000000000000000000000000008")]
    fn address_frozen_event(&self, address: &Address, _data: ());

    #[event("0x0000000000000000000000000000000000000000000000000000000000000009")]
    fn address_unfrozen_event(&self, address: &Address, _data: ());

    #[event("0x000000000000000000000000000000000000000000000000000000000000000a")]
    fn frozen_address_wiped_event(&self, address: &Address, _data: ());

    #[event("0x000000000000000000000000000000000000000000000000000000000000000b")]
    fn asset_protection_role_set_event(&self, 
        old_asset_protection_role: &Address,
        new_asset_protection_role: &Address,
        _data: ());

    // SUPPLY CONTROL EVENTS

    #[event("0x000000000000000000000000000000000000000000000000000000000000000c")]
    fn supply_increased_event(&self, to: &Address, amount: &BigUint);

    #[event("0x000000000000000000000000000000000000000000000000000000000000000d")]
    fn supply_decreased_event(&self, from: &Address, amount: &BigUint);

    #[event("0x000000000000000000000000000000000000000000000000000000000000000e")]
    fn supply_controller_set_event(&self, 
        old_supply_controller: &Address,
        new_supply_controller: &Address,
        _data: ());
    
}
