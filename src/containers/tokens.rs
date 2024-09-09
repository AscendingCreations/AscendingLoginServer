use chrono::{DateTime, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation};
use rand::Rng;
use serde::{de::DeserializeOwned, Serialize};

pub const KEY_LENGTH: usize = 64;
pub const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
    abcdefghijklmnopqrstuvwxyz\
    0123456789)(*&^%$#@!~";

pub fn gen_token(length: usize) -> String {
    let mut rng = rand::thread_rng();

    (0..length)
        .map(|_| {
            let index = rng.gen_range(0..CHARSET.len());
            CHARSET[index] as char
        })
        .collect()
}

#[derive(Clone)]
pub struct RotatableKey {
    keys: [String; 2],
    active_id: usize,
    last_updated: DateTime<Utc>,
}

impl RotatableKey {
    pub fn new(key_size: usize) -> Self {
        let keys = [gen_token(key_size), gen_token(key_size)];

        Self {
            keys,
            active_id: 0,
            last_updated: Utc::now(),
        }
    }

    pub fn keys(&self) -> [&str; 2] {
        let keys = self.keys.as_ref();

        [keys[0].as_str(), keys[1].as_str()]
    }

    #[inline(always)]
    pub fn active_id(&self) -> usize {
        self.active_id
    }

    #[inline(always)]
    pub fn inactive_id(&self) -> usize {
        (self.active_id + 1) % 2
    }

    pub fn active_key(&self) -> &str {
        self.keys[self.active_id()].as_str()
    }

    pub fn inactive_key(&self) -> &str {
        self.keys[self.inactive_id()].as_str()
    }

    pub fn last_updated(&self) -> DateTime<Utc> {
        self.last_updated
    }

    pub fn rotate(&mut self) {
        self.active_id = (self.active_id + 1) % 2;

        let key = loop {
            let key = gen_token(self.keys[0].len());

            if key == self.keys[0] || key == self.keys[1] {
                continue;
            }

            break key;
        };

        self.keys[self.active_id] = key;
        self.last_updated = Utc::now();
    }
}

#[derive(Clone)]
pub struct RotatableJwtKey {
    keys: [Key; 2],
    rotatable_key: RotatableKey,
}

impl RotatableJwtKey {
    pub fn new(key_size: usize) -> Self {
        let rotatable_key = RotatableKey::new(key_size);
        let keys = rotatable_key.keys();

        let keys = [
            Key::from_secret(keys[0].as_bytes()),
            Key::from_secret(keys[1].as_bytes()),
        ];

        Self {
            keys,
            rotatable_key,
        }
    }

    #[inline(always)]
    pub fn active_id(&self) -> usize {
        self.rotatable_key.active_id()
    }

    #[inline(always)]
    pub fn inactive_id(&self) -> usize {
        self.rotatable_key.inactive_id()
    }

    pub fn active_key(&self) -> &Key {
        &self.keys[self.active_id()]
    }

    pub fn inactive_key(&self) -> &Key {
        &self.keys[self.inactive_id()]
    }

    pub fn last_updated(&self) -> DateTime<Utc> {
        self.rotatable_key.last_updated()
    }

    pub fn encode<T: Serialize>(&self, header: &Header, data: &T) -> crate::Result<String> {
        let key = self.active_key();
        Ok(jsonwebtoken::encode(header, data, key.encoding_key())?)
    }

    pub fn decode<T: DeserializeOwned>(
        &self,
        data: &str,
        validation: &Validation,
    ) -> Option<(usize, TokenData<T>)> {
        for (index, key) in self.keys.iter().enumerate() {
            if let Ok(result) = jsonwebtoken::decode::<T>(data, key.decoding_key(), validation) {
                return Some((index, result));
            }
        }

        None
    }

    pub fn rotate(&mut self) {
        self.rotatable_key.rotate();
        let active_id = self.rotatable_key.active_id();
        self.keys[active_id] = Key::from_secret(self.rotatable_key.active_key().as_bytes());
    }
}

#[derive(Clone)]
pub struct Key {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl Key {
    pub fn from_secret(bytes: &[u8]) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(bytes),
            decoding_key: DecodingKey::from_secret(bytes),
        }
    }

    pub fn encoding_key(&self) -> &EncodingKey {
        &self.encoding_key
    }

    pub fn decoding_key(&self) -> &DecodingKey {
        &self.decoding_key
    }
}
