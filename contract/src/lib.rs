use crate::KeyStore::Rooms;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::env::predecessor_account_id;
use near_sdk::serde::Serialize;
use near_sdk::store::LookupMap;
use near_sdk::store::Vector;
use near_sdk::BorshStorageKey;
use near_sdk::{near_bindgen, AccountId, CryptoHash};

type RoomId = u64;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Room {
    room_id: RoomId,
    owner_id: AccountId,
    players: Vector<AccountId>,
    banned_players: Vector<AccountId>,
    player_limit: u32,
    is_hidden: bool,
    is_closed: bool,
}

#[near_bindgen]
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct RoomConfig {
    is_hidden: bool,
    player_limit: u32,
}

#[derive(BorshStorageKey, BorshSerialize)]
pub enum KeyStore {
    Rooms,
    BannedPlayers { account_room_hash: CryptoHash },
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
    rooms: LookupMap<RoomId, Room>,
    next_room_id: u64,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            rooms: LookupMap::new(Rooms),
            next_room_id: 0,
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn create_room(&mut self, room_config: RoomConfig) {
        let account_id = predecessor_account_id();

        let next_room_id = self.next_room_id;
        let room_id_hash = next_room_id.to_le_bytes();
        let room_prefix: Vec<u8> = [
            b"s".as_slice(),
            &near_sdk::env::sha256_array(
                [&account_id.as_bytes()[..], &room_id_hash[..]]
                    .concat()
                    .as_slice(),
            ),
        ]
        .concat();

        let players_prefix: Vec<u8> =
            [b"s".as_slice(), &near_sdk::env::sha256_array(&room_id_hash)].concat();

        let new_room = Room {
            room_id: next_room_id,
            owner_id: account_id,
            players: Vector::new(players_prefix),
            banned_players: Vector::new(room_prefix),
            player_limit: room_config.player_limit,
            is_hidden: room_config.is_hidden,
            is_closed: false,
        };

        self.rooms.insert(new_room.room_id, new_room);
        self.next_room_id += 1;
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
