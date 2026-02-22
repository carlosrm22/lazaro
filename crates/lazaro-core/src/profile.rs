use std::collections::BTreeMap;

use crate::config::Settings;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub settings: Settings,
}

impl Profile {
    pub fn new(id: impl Into<String>, name: impl Into<String>, settings: Settings) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            settings,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProfileStore {
    profiles: BTreeMap<String, Profile>,
    active_id: Option<String>,
}

impl ProfileStore {
    pub fn upsert(&mut self, profile: Profile) {
        let id = profile.id.clone();
        self.profiles.insert(id.clone(), profile);
        if self.active_id.is_none() {
            self.active_id = Some(id);
        }
    }

    pub fn remove(&mut self, id: &str) -> Option<Profile> {
        let removed = self.profiles.remove(id);
        if self.active_id.as_deref() == Some(id) {
            self.active_id = self.profiles.keys().next().cloned();
        }
        removed
    }

    pub fn activate(&mut self, id: &str) -> bool {
        if self.profiles.contains_key(id) {
            self.active_id = Some(id.to_string());
            true
        } else {
            false
        }
    }

    pub fn active(&self) -> Option<&Profile> {
        self.active_id
            .as_deref()
            .and_then(|id| self.profiles.get(id))
    }

    pub fn list(&self) -> Vec<&Profile> {
        self.profiles.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_switches_profile() {
        let mut store = ProfileStore::default();
        let a = Profile::new("office", "Office", Settings::default());
        let mut gaming = Settings::default();
        gaming.micro.interval_seconds = 300;
        let b = Profile::new("gaming", "Gaming", gaming);

        store.upsert(a);
        store.upsert(b);

        assert!(store.activate("gaming"));
        let active = store.active().expect("active profile must exist");
        assert_eq!(active.id, "gaming");
        assert_eq!(active.settings.micro.interval_seconds, 300);
    }
}
