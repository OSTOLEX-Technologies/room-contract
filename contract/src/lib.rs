use crate::KeyStore::{AppRooms, Rooms, RoomsPerApp, RoomsPerAppOwner, RoomsPerOwner};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::env::{predecessor_account_id, random_seed};
use near_sdk::json_types::U128;
use near_sdk::serde::Serialize;
use near_sdk::store::{LookupMap, UnorderedSet};
use near_sdk::BorshStorageKey;
use near_sdk::{near_bindgen, AccountId, CryptoHash};

type RoomId = u64;
type AppName = String;

#[derive(Clone, BorshDeserialize, BorshSerialize)]
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
#[derive(Serialize)]
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
    AppRooms { hash: CryptoHash },
    RoomsPerAppOwner,
    RoomsPerOwner { hash: CryptoHash },
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
    rooms: LookupMap<RoomId, Room>,
    available_rooms_per_app: UnorderedMap<AppName, UnorderedSet<RoomId>>,
    rooms_per_app_owner: UnorderedMap<AppName, LookupMap<AccountId, Vec<RoomId>>>,
    next_room_id: u64,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            rooms: LookupMap::new(Rooms),
            available_rooms_per_app: UnorderedMap::new(RoomsPerApp),
            rooms_per_app_owner: UnorderedMap::new(RoomsPerAppOwner),
            next_room_id: 0,
        }
    }
}

#[near_bindgen]
impl Contract {
    pub fn get_app_rooms(
        &self,
        app_name: AppName,
        from_index: Option<U128>,
        limit: Option<usize>,
    ) -> Vec<Room> {
        let app_rooms = self
            .available_rooms_per_app
            .get(&app_name)
            .expect("App rooms not found");
        let start = u128::from(from_index.unwrap_or(U128(0)));

        app_rooms
            .iter()
            .skip(start as usize)
            .take(limit.unwrap_or(0))
            .map(|x| self.rooms.get(x).expect("Room not found").clone())
            .collect()
    }

    pub fn get_random_room(&self, app_name: AppName) -> Room {
        let app_rooms = self
            .available_rooms_per_app
            .get(&app_name)
            .expect("App rooms not found");

        let room_ids: Vec<&RoomId> = app_rooms.iter().collect();
        let number_of_rooms = room_ids.len() as usize;
        let rnd_idx = self.get_random_in_range(0, number_of_rooms, 0);
        let rnd_room_id = room_ids.get(rnd_idx).expect("Random room id not found");

        self.rooms
            .get(rnd_room_id)
            .expect("Random room not found")
            .clone()
    }

    pub fn get_random_in_range(&self, min: usize, max: usize, index: usize) -> usize {
        let random = *random_seed().get(index).unwrap();
        let random_in_range = (random as f64 / 256.0) * (max - min) as f64 + min as f64;
        random_in_range.floor() as usize
    }

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
            players: vec![account_id.clone()],
            banned_players: Vec::new(),
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

        let mut rooms_per_app = self
            .available_rooms_per_app
            .get(&room_config.app_name)
            .unwrap_or_else(|| UnorderedSet::new(AppRooms { hash }));

        rooms_per_app.insert(room_id);
        self.available_rooms_per_app
            .insert(&room_config.app_name, &rooms_per_app);

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
