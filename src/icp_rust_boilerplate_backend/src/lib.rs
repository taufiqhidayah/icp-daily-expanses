#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Expense {
    id: u64,
    description: String,
    amount: f64,
    date: u64, // Timestamp of when the expense was made
    created_at: u64,
    updated_at: Option<u64>,
}

// Implementing `Storable` trait for `Expense`
impl Storable for Expense {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// Implementing `BoundedStorable` trait for `Expense`
impl BoundedStorable for Expense {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static STORAGE: RefCell<StableBTreeMap<u64, Expense, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct ExpensePayload {
    description: String,
    amount: f64,
    date: u64, // Timestamp of the expense
}

#[ic_cdk::query]
fn get_expense(id: u64) -> Result<Expense, Error> {
    match _get_expense(&id) {
        Some(expense) => Ok(expense),
        None => Err(Error::NotFound {
            msg: format!("Expense with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn add_expense(payload: ExpensePayload) -> Option<Expense> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");

    let new_expense = Expense {
        id,
        description: payload.description,
        amount: payload.amount,
        date: payload.date,
        created_at: time(),
        updated_at: None,
    };
    do_insert(&new_expense);
    Some(new_expense)
}

#[ic_cdk::update]
fn update_expense(id: u64, payload: ExpensePayload) -> Result<Expense, Error> {
    match STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut expense) => {
            expense.description = payload.description;
            expense.amount = payload.amount;
            expense.date = payload.date;
            expense.updated_at = Some(time());
            do_insert(&expense);
            Ok(expense)
        }
        None => Err(Error::NotFound {
            msg: format!("Couldn't update expense with id={}. Expense not found.", id),
        }),
    }
}

#[ic_cdk::update]
fn delete_expense(id: u64) -> Result<Expense, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(expense) => Ok(expense),
        None => Err(Error::NotFound {
            msg: format!("Couldn't delete expense with id={}. Expense not found.", id),
        }),
    }
}

// Helper function to perform the insertion
fn do_insert(expense: &Expense) {
    STORAGE.with(|service| service.borrow_mut().insert(expense.id, expense.clone()));
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

// Helper method to get an expense by id
fn _get_expense(id: &u64) -> Option<Expense> {
    STORAGE.with(|service| service.borrow().get(id))
}

// Export candid for the canister
ic_cdk::export_candid!();
