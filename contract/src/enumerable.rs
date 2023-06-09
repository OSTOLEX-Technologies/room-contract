use crate::*;

#[near_bindgen]
impl Contract {
    pub fn get_app_account_room(&self, app_name: AppName, account_id: AccountId) -> Option<Room> {
        let wrapped_room_per_account = self
            .rooms_per_app_account
            .get(&app_name);
        if wrapped_room_per_account.is_none() {
            return None;
        }
        let room_per_account = wrapped_room_per_account.unwrap();

        match room_per_account.get(&account_id) {
            None => None,
            Some(room_id_opt) => {
                room_id_opt.map(|room_id| self.rooms.get(&room_id).expect("").clone())
            }
        }
    }

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

    pub fn get_number_of_available_rooms(&self, app_name: AppName) -> usize {
        let wrapped_app_rooms = self
            .available_rooms_per_app
            .get(&app_name);

        if wrapped_app_rooms.is_none() {
            return 0;
        }

        let app_rooms = wrapped_app_rooms.unwrap();

        let room_ids: Vec<&RoomId> = app_rooms.iter().collect();
        room_ids.len()
    }

    pub fn get_random_room(&self, app_name: AppName) -> Room {
        let app_rooms = self
            .available_rooms_per_app
            .get(&app_name)
            .expect("App rooms not found");

        let room_ids: Vec<&RoomId> = app_rooms.iter().collect();
        let number_of_rooms = room_ids.len() as usize;
        if number_of_rooms == 0 {
            panic!("There are currently no available rooms")
        }

        let rnd_idx = self.get_random_in_range(0, number_of_rooms, 0);
        let rnd_room_id = room_ids.get(rnd_idx).expect("Random room id not found");

        let random_room = self.rooms
            .get(rnd_room_id)
            .expect("Random room not found")
            .clone();

        random_room
    }

    pub fn get_random_in_range(&self, min: usize, max: usize, index: usize) -> usize {
        let random = *random_seed().get(index).unwrap();
        let random_in_range = (random as f64 / 256.0) * (max - min) as f64 + min as f64;
        random_in_range.floor() as usize
    }
}
