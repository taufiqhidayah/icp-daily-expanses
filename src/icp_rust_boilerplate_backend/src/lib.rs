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
struct Vote {
    id: u64,
    candidate: String,
    voter_id: String,
    count: u64,
    created_at: u64,
    updated_at: Option<u64>,
}

impl Storable for Vote {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Vote {
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

    static VOTE_STORAGE: RefCell<StableBTreeMap<u64, Vote, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct VotePayload {
    candidate: String,
    voter_id: String,
}

#[ic_cdk::query]
fn get_vote(id: u64) -> Result<Vote, VotingError> {
    match _get_vote(&id) {
        Some(vote) => Ok(vote),
        None => Err(VotingError::NotFound {
            msg: format!("Vote with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn add_vote(vote: VotePayload) -> Option<Vote> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");

    let new_vote = Vote {
        id,
        candidate: vote.candidate,
        voter_id: vote.voter_id,
        count: 1, // Initial vote count set to 1
        created_at: time(),
        updated_at: None,
    };
    do_insert(&new_vote);
    Some(new_vote)
}

#[ic_cdk::update]
fn update_vote(id: u64, payload: VotePayload) -> Result<Vote, VotingError> {
    match VOTE_STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut vote) => {
            vote.candidate = payload.candidate;
            vote.voter_id = payload.voter_id;
            vote.count += 1; // Increment the vote count
            vote.updated_at = Some(time());
            do_insert(&vote);
            Ok(vote)
        }
        None => Err(VotingError::NotFound {
            msg: format!(
                "Couldn't update vote with id={}. Vote not found",
                id
            ),
        }),
    }
}

fn do_insert(vote: &Vote) {
    VOTE_STORAGE.with(|service| service.borrow_mut().insert(vote.id, vote.clone()));
}

#[ic_cdk::update]
fn delete_vote(id: u64) -> Result<Vote, VotingError> {
    match VOTE_STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(vote) => Ok(vote),
        None => Err(VotingError::NotFound {
            msg: format!(
                "Couldn't delete vote with id={}. Vote not found.",
                id
            ),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum VotingError {
    NotFound { msg: String },
}

fn _get_vote(id: &u64) -> Option<Vote> {
    VOTE_STORAGE.with(|service| service.borrow().get(id))
}

ic_cdk::export_candid!();
