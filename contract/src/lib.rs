mod account;
mod enumerable;
mod storage_tracker;

use crate::account::Account;
use crate::KeyStore::{
    Accounts, AppRooms, Rooms, RoomsPerAccount, RoomsPerApp, RoomsPerAppAccount, StorageDeposit,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::env::{attached_deposit, predecessor_account_id, random_seed};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::store::{LookupMap, UnorderedSet};
use near_sdk::{near_bindgen, AccountId, CryptoHash};
use near_sdk::{Balance, BorshStorageKey, Promise};
use near_sys::panic;

type RoomId = u64;
type AppName = String;

#[near_bindgen]
#[derive(Clone, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Room {
    room_id: RoomId,
    name: String,
    owner_id: AccountId,
    players: Vec<AccountId>,
    banned_players: Vec<AccountId>,
    player_limit: usize,
    is_hidden: bool,
    is_closed: bool,
    extra: Option<String>,
}

#[near_bindgen]
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct RoomConfig {
    app_name: String,
    name: String,
    is_hidden: bool,
    player_limit: usize,
    extra: Option<String>,
}

#[derive(BorshStorageKey, BorshSerialize)]
pub enum KeyStore {
    Rooms,
    RoomsPerApp,
    Accounts,
    AppRooms { hash: CryptoHash },
    RoomsPerAppAccount,
    RoomsPerAccount { hash: CryptoHash },
    StorageDeposit,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
    rooms: LookupMap<RoomId, Room>,
    accounts: LookupMap<AccountId, Account>,
    available_rooms_per_app: UnorderedMap<AppName, UnorderedSet<RoomId>>,
    rooms_per_app_account: UnorderedMap<AppName, LookupMap<AccountId, Option<RoomId>>>,
    storage_deposits: LookupMap<AccountId, Balance>,
    next_room_id: u64,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            rooms: LookupMap::new(Rooms),
            accounts: LookupMap::new(Accounts),
            available_rooms_per_app: UnorderedMap::new(RoomsPerApp),
            rooms_per_app_account: UnorderedMap::new(RoomsPerAppAccount),
            storage_deposits: LookupMap::new(StorageDeposit),
            next_room_id: 0,
        }
    }
}

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn create_room(&mut self, room_config: RoomConfig) -> RoomId {
        let account_id = predecessor_account_id();
        let room_id = self.next_room_id;

        let new_room = Room {
            room_id,
            name: room_config.name.clone(),
            owner_id: account_id.clone(),
            players: vec![account_id.clone()],
            banned_players: Vec::new(),
            player_limit: room_config.player_limit.clone(),
            is_hidden: room_config.is_hidden.clone(),
            is_closed: false,
            extra: room_config.extra.clone(),
        };

        let attached_balanced = attached_deposit();
        let mut account = self.internal_unwrap_account_or_create(&account_id, attached_balanced);
        account.start_storage_tracker();

        self.save_new_room(new_room, &room_config, &account_id);
        self.next_room_id += 1;

        account.stop_storage_tracker();
        self.internal_set_account(&account_id, account);

        room_id
    }

    fn save_new_room(&mut self, new_room: Room, room_config: &RoomConfig, account_id: &AccountId) {
        let room_id_hash = new_room.room_id.to_le_bytes();

        let hash = near_sdk::env::sha256_array(
            [&account_id.as_bytes()[..], &room_id_hash[..]]
                .concat()
                .as_slice(),
        );

        let mut rooms_per_account = self
            .rooms_per_app_account
            .get(&room_config.app_name)
            .unwrap_or_else(|| LookupMap::new(RoomsPerAccount { hash }));

        rooms_per_account.insert(account_id.clone(), Some(new_room.room_id.clone()));

        self.rooms_per_app_account
            .insert(&room_config.app_name, &rooms_per_account);

        let mut rooms_per_app = self
            .available_rooms_per_app
            .get(&room_config.app_name)
            .unwrap_or_else(|| UnorderedSet::new(AppRooms { hash }));

        rooms_per_app.insert(new_room.room_id.clone());
        self.available_rooms_per_app
            .insert(&room_config.app_name, &rooms_per_app);

        self.rooms.insert(new_room.room_id, new_room);
    }

    pub fn random_join(&mut self, app_name: AppName) -> RoomId {
        let account_id = predecessor_account_id();
        let room_per_account = self.rooms_per_app_account.get(&app_name).expect("App not found");
        let room = room_per_account.get(&account_id);
        if !room.is_none() {
            panic!("Account is already in the room")
        }

        let random_room = self.get_random_room(app_name.clone());
        self.join(random_room.room_id.clone(), app_name);

        random_room.room_id
    }

    pub fn join(&mut self, room_id: RoomId, app_name: AppName) {
        let room = self.rooms.get_mut(&room_id).expect("Room id not found");
        if room.is_closed {
            panic!("The room is already closed")
        }

        if room.player_limit <= room.players.len() {
            panic!("Player limit exceeded")
        }
        let player_id = predecessor_account_id();
        if room.players.contains(&player_id) {
            panic!("The player is already joined")
        }

        for banned_player_id in room.banned_players.iter() {
            if banned_player_id.eq(&player_id) {
                panic!("Player is banned")
            }
        }

        let mut room_per_account = self
            .rooms_per_app_account
            .get(&app_name)
            .expect("App not found");

        room_per_account.insert(player_id.clone(), Some(room_id));
        self.rooms_per_app_account
            .insert(&app_name, &room_per_account);
        room.players.push(player_id);
    }

    pub fn leave(&mut self, room_id: RoomId, app_name: AppName) {
        let room = self.rooms.get_mut(&room_id).expect("Room id not found");
        if room.is_closed {
            panic!("The room is already closed")
        }

        let mut room_per_account = self
            .rooms_per_app_account
            .get(&app_name)
            .expect("App not found");

        let player_leave_id = predecessor_account_id();
        let mut player_idx = 0;
        for player_id in room.players.iter() {
            if player_id.eq(&player_leave_id) {
                room_per_account.insert(player_id.clone(), None);
                self.rooms_per_app_account
                    .insert(&app_name, &room_per_account);
                room.players.swap_remove(player_idx);
                return;
            }

            player_idx += 1;
        }
    }

    pub fn open(&mut self, room_id: RoomId, app_name: AppName) {
        let player_id = predecessor_account_id();
        let mut room = self.rooms.get_mut(&room_id).expect("Room id not found");

        if room.owner_id.ne(&player_id) {
            panic!("Only the owner can open the room")
        }

        room.is_closed = false;

        let mut available_rooms = self
            .available_rooms_per_app
            .get(&app_name)
            .expect("Available rooms not found in the app");

        available_rooms.insert(room_id);

        self.available_rooms_per_app
            .insert(&app_name, &available_rooms);
    }

    pub fn close(&mut self, room_id: RoomId, app_name: AppName) {
        let mut room = self.rooms.get_mut(&room_id).expect("Room id not found");
        if room.is_closed {
            panic!("The room is already closed")
        }

        let player_id = predecessor_account_id();

        if room.owner_id.ne(&player_id) {
            panic!("Only the owner can close the room")
        }

        room.is_closed = true;

        self.remove_room_from_available(&room_id, &app_name);
    }

    fn remove_room_from_available(&mut self, room_id: &RoomId, app_name: &AppName) {
        let mut available_rooms = self
            .available_rooms_per_app
            .get(&app_name)
            .expect("Available rooms not found in the app");

        if !available_rooms.remove(&room_id) {
            panic!("Room not found in the app");
        }

        self.available_rooms_per_app
            .insert(&app_name, &available_rooms);
    }

    pub fn remove(&mut self, room_id: RoomId, app_name: AppName) {
        let room = self.rooms.get(&room_id).expect("Room id not found");
        let player_id = predecessor_account_id();

        if room.owner_id.ne(&player_id) {
            panic!("Only the owner can remove the room")
        }

        let mut room_per_account = self
            .rooms_per_app_account
            .get(&app_name)
            .expect("App name not found");

        for player_id in &room.players {
            room_per_account.insert(player_id.clone(), None);
        }
        self.rooms_per_app_account.insert(&app_name, &room_per_account);
        self.rooms.remove(&room_id);
        self.remove_room_from_available(&room_id, &app_name);
    }

    pub fn kick_and_ban(&mut self, player_to_ban_id: AccountId, room_id: RoomId) {
        let room = self.rooms.get_mut(&room_id).expect("Room id not found");
        if room.is_closed {
            panic!("The room is already closed")
        }

        let player_id = predecessor_account_id();

        if room.owner_id.ne(&player_id) {
            panic!("Only the owner can kick the player")
        }

        room.players.retain(|x| x.ne(&player_to_ban_id));
        room.banned_players.push(player_to_ban_id.clone());
    }
}
