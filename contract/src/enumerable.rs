use crate::*;

#[near_bindgen]
impl Contract {
    pub fn get_owner_app_rooms(
        &self,
        app_name: &AppName,
        owner_id: AccountId,
        from_index: Option<U128>,
        limit: Option<usize>,
    ) -> Vec<Room> {
        let rooms_per_owner = self
            .rooms_per_app_owner
            .get(&app_name)
            .expect("App not found");

        let room_ids = rooms_per_owner
            .get(&owner_id)
            .expect("Owner rooms not found");

        let start = u128::from(from_index.unwrap_or(U128(0)));

        room_ids
            .iter()
            .skip(start as usize)
            .take(limit.unwrap_or(0))
            .map(|room_id| self.rooms.get(room_id).expect("Room not found").clone())
            .collect()
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
}
