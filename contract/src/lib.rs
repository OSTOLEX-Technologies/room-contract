use crate::KeyStore::{BannedPlayers, Players, Rooms, RoomsPerApp, RoomsPerOwner};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::env::predecessor_account_id;
use near_sdk::serde::Serialize;
use near_sdk::store::LookupMap;
use near_sdk::store::Vector;
use near_sdk::BorshStorageKey;
use near_sdk::{near_bindgen, AccountId, CryptoHash};

type RoomId = u64;
type AppName = String;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Room {
    room_id: RoomId,
    name: String,
    owner_id: AccountId,
    players: Vector<AccountId>,
    banned_players: Vector<AccountId>,
    player_limit: u32,
    is_hidden: bool,
    is_closed: bool,
    extra: Option<String>,
}

#[near_bindgen]
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct RoomConfig {
    app_name: String,
    name: String,
    is_hidden: bool,
    player_limit: u32,
    extra: Option<String>,
}

#[derive(BorshStorageKey, BorshSerialize)]
pub enum KeyStore {
    Rooms,
    RoomsPerApp,
    RoomsPerOwner { hash: CryptoHash },
    Players { hash: CryptoHash },
    BannedPlayers { hash: CryptoHash },
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
    rooms: LookupMap<RoomId, Room>,
    rooms_per_app_owner: UnorderedMap<AppName, LookupMap<AccountId, Vec<RoomId>>>,
    next_room_id: u64,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            rooms: LookupMap::new(Rooms),
            rooms_per_app_owner: UnorderedMap::new(RoomsPerApp),
            next_room_id: 0,
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn create_room(&mut self, room_config: RoomConfig) -> RoomId {
        let account_id = predecessor_account_id();

        let room_id = self.next_room_id;
        let room_id_hash = room_id.to_le_bytes();

        let hash = near_sdk::env::sha256_array(
            [&account_id.as_bytes()[..], &room_id_hash[..]]
                .concat()
                .as_slice(),
        );

        let new_room = Room {
            room_id,
            name: room_config.name,
            owner_id: account_id.clone(),
            players: Vector::new(Players { hash: hash.clone() }),
            banned_players: Vector::new(BannedPlayers { hash: hash.clone() }),
            player_limit: room_config.player_limit,
            is_hidden: room_config.is_hidden,
            is_closed: false,
            extra: room_config.extra,
        };

        self.rooms.insert(new_room.room_id, new_room);

        let mut rooms_per_owner = self
            .rooms_per_app_owner
            .get(&room_config.app_name)
            .unwrap_or_else(|| LookupMap::new(RoomsPerOwner { hash }));

        let mut rooms = match rooms_per_owner.get(&account_id) {
            None => Vec::new(),
            Some(rooms) => rooms.clone(),
        };

        rooms.push(room_id);
        rooms_per_owner.insert(account_id.clone(), rooms.clone());

        self.rooms_per_app_owner
            .insert(&room_config.app_name, &rooms_per_owner);

        self.next_room_id += 1;

        room_id
    }

    pub fn join(&mut self, room_id: RoomId) {
        let room = self.rooms.get_mut(&room_id).expect("Room id not found");
        if room.is_closed {
            panic!("The room is already closed")
        }

        if room.player_limit >= room.players.len() {
            panic!("Player limit exceeded")
        }

        let player_id = predecessor_account_id();
        for banned_player_id in room.banned_players.iter() {
            if banned_player_id.eq(&player_id) {
                panic!("Player is banned")
            }
        }

        room.players.push(player_id);
    }

    pub fn leave(&mut self, room_id: RoomId) {
        let room = self.rooms.get_mut(&room_id).expect("Room id not found");
        if room.is_closed {
            panic!("The room is already closed")
        }

        let player_leave_id = predecessor_account_id();
        let mut player_idx = 0;
        for player_id in room.players.iter() {
            if player_id.eq(&player_leave_id) {
                room.players.swap_remove(player_idx);
                return;
            }

            player_idx += 1;
        }
    }

    pub fn close(&mut self, room_id: RoomId) {
        let mut room = self.rooms.get_mut(&room_id).expect("Room id not found");
        if room.is_closed {
            panic!("The room is already closed")
        }

        let player_id = predecessor_account_id();

        if room.owner_id.ne(&player_id) {
            panic!("Only the owner can close the room")
        }

        room.is_closed = true;
    }

    pub fn remove(&mut self, room_id: RoomId, app_name: AppName) {
        let room = self.rooms.get_mut(&room_id).expect("Room id not found");
        let player_id = predecessor_account_id();

        if room.owner_id.ne(&player_id) {
            panic!("Only the owner can close the room")
        }

        let mut rooms_per_owner = self
            .rooms_per_app_owner
            .get(&app_name)
            .expect("App name not found");

        let mut player_rooms = rooms_per_owner
            .get(&player_id)
            .expect("Player rooms not found")
            .clone();

        let mut room_idx = 0;
        for room_id_to_remove in player_rooms.iter() {
            if room_id_to_remove.eq(&room_id) {
                player_rooms.swap_remove(room_idx);
                rooms_per_owner.insert(player_id, player_rooms);
                self.rooms_per_app_owner.insert(&app_name, &rooms_per_owner);
                self.rooms.remove(&room_id);
                return;
            }

            room_idx += 1;
        }
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

        let mut player_to_ban_index = 0;
        for player_id in room.players.iter() {
            if player_id.eq(&player_to_ban_id.clone()) {
                room.banned_players.push(player_to_ban_id.clone());
                room.players.swap_remove(player_to_ban_index);
                return;
            }

            player_to_ban_index += 1;
        }
    }
}
