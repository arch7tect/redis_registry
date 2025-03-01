// redis_registry.rs
use redis::{AsyncCommands, Client, RedisError, RedisResult};
use rocket::serde::json::Value as JsonValue;
use serde_json::Value;
use std::env;
use std::sync::Arc;
// =======================================================
// Redis Registry Core Implementation (Internal API)
// =======================================================

pub struct RedisRegistry {
    client: Client,
    owner_type: String,
    owner_id: String,
}

fn value_to_string(value: &Value) -> Result<String, RedisError> {
    trace!("Serializing JSON value");
    serde_json::to_string(&value).map_err(|e| {
        error!("Failed to serialize JSON: {}", e);
        RedisError::from(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to serialize JSON: {}", e),
        ))
    })
}

fn string_to_value(value_str: &String) -> Result<Value, RedisError> {
    trace!("Deserializing JSON string");
    serde_json::from_str(&value_str).map_err(|e| {
        error!("Failed to deserialize JSON: {}", e);
        RedisError::from(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to deserialize JSON: {}", e),
        ))
    })
}

impl RedisRegistry {
    /// Create a new RedisRegistry instance using environment variables
    pub fn new(owner_type: &str, owner_id: &str) -> Result<Self, RedisError> {
        debug!(
            "Creating new RedisRegistry with owner_type={}, owner_id={}",
            owner_type, owner_id
        );

        let redis_url = env::var("REDIS_URL").ok();
        let redis_host = env::var("REDIS_HOST").ok();
        let redis_port = env::var("REDIS_PORT").ok();

        let redis_url = match (redis_url, redis_host, redis_port) {
            (Some(url), _, _) => {
                debug!("Using REDIS_URL: {}", url);
                url
            }
            (_, Some(host), Some(port)) => {
                let url = format!("redis://{}:{}", host, port);
                debug!(
                    "Using constructed URL from REDIS_HOST and REDIS_PORT: {}",
                    url
                );
                url
            }
            (_, Some(host), None) => {
                let url = format!("redis://{}:6379", host);
                debug!(
                    "Using constructed URL from REDIS_HOST with default port: {}",
                    url
                );
                url
            }
            _ => {
                error!("Redis connection information not provided");
                return Err(RedisError::from(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Redis connection information not provided. Set REDIS_URL or REDIS_HOST and REDIS_PORT",
                )));
            }
        };

        let client = match Client::open(redis_url.clone()) {
            Ok(client) => {
                info!("Successfully connected to Redis at {}", redis_url);
                client
            }
            Err(e) => {
                error!("Failed to connect to Redis at {}: {}", redis_url, e);
                return Err(e);
            }
        };

        Ok(RedisRegistry {
            client,
            owner_type: owner_type.to_string(),
            owner_id: owner_id.to_string(),
        })
    }

    /// Get a Redis connection
    async fn get_connection(&self) -> RedisResult<redis::aio::MultiplexedConnection> {
        trace!("Getting Redis connection");
        match self.client.get_multiplexed_async_connection().await {
            Ok(conn) => {
                trace!("Redis connection acquired");
                Ok(conn)
            }
            Err(e) => {
                error!("Failed to get Redis connection: {}", e);
                Err(e)
            }
        }
    }

    /// Get the owner prefix (/<owner_type>/<owner_id>)
    fn get_owner_prefix(&self) -> String {
        format!("/{}/{}", self.owner_type, self.owner_id)
    }

    /// Build a key from parts with the owner prefix: /<owner_type>/<owner_id>/<part1>/<part2>/...
    /// The owner_type and owner_id are hidden implementation details and not exposed to API users
    fn build_key(&self, parts: &Vec<String>) -> String {
        if parts.is_empty() {
            let key = self.get_owner_prefix();
            trace!("Built key (root): {}", key);
            key
        } else {
            let key = format!("{}/{}", self.get_owner_prefix(), parts.join("/"));
            trace!("Built key: {}", key);
            key
        }
    }

    /// Set a value for the specified key parts
    pub async fn set(&self, parts: &Vec<String>, value: JsonValue) -> RedisResult<()> {
        let key = self.build_key(parts);
        info!("Setting value for key: {}", key);

        let value_str = value_to_string(&value)?;
        let mut conn = self.get_connection().await?;

        // Execute the command and capture the result
        let result = conn.set(&key, &value_str).await;

        // Log based on the result
        match &result {
            Ok(_) => debug!("Successfully set value for key: {}", key),
            Err(e) => error!("Redis SET operation failed for key {}: {}", key, e),
        }

        result
    }

    /// Get the value for the specified key parts
    pub async fn get(&self, parts: &Vec<String>) -> RedisResult<Option<JsonValue>> {
        let key = self.build_key(parts);
        info!("Getting value for key: {}", key);

        let mut conn = self.get_connection().await?;
        let value_result: RedisResult<Option<String>> = conn.get(&key).await;

        match &value_result {
            Ok(Some(_)) => debug!("Redis GET operation successful for key: {}", key),
            Ok(None) => debug!("No value found for key: {}", key),
            Err(e) => error!("Redis GET operation failed for key {}: {}", key, e),
        }

        // Handle errors from the Redis GET operation
        let value = value_result?;

        if let Some(value_str) = value {
            let json_result = string_to_value(&value_str);

            match &json_result {
                Ok(_) => trace!("Successfully deserialized JSON for key: {}", key),
                Err(e) => error!("Failed to deserialize JSON for key {}: {}", key, e),
            }

            json_result.map(Some)
        } else {
            Ok(None)
        }
    }

    /// Delete the key specified by parts
    pub async fn delete(&self, parts: &Vec<String>) -> RedisResult<bool> {
        let key = self.build_key(parts);
        info!("Deleting key: {}", key);

        let mut conn = self.get_connection().await?;
        let deleted_result: RedisResult<i32> = conn.del(&key).await;

        match &deleted_result {
            Ok(count) => {
                if *count > 0 {
                    info!("Key deleted: {}", key);
                } else {
                    debug!("Key not found for deletion: {}", key);
                }
            }
            Err(e) => error!("Redis DEL operation failed for key {}: {}", key, e),
        }

        // Convert the result count to a boolean success indicator
        deleted_result.map(|count| count > 0)
    }

    /// Delete all keys that start with the specified parts
    pub async fn purge(&self, parts: &Vec<String>) -> RedisResult<i64> {
        info!("Purging keys with prefix: {:?}", parts);

        let mut conn = self.get_connection().await?;
        let keys = match self.scan(parts).await {
            Ok(k) => k,
            Err(e) => {
                error!("Failed to scan keys for purge operation: {}", e);
                return Err(e);
            }
        };

        info!("Found {} keys to purge", keys.len());

        if keys.is_empty() {
            return Ok(0);
        }

        let full_keys: Vec<String> = keys
            .into_iter()
            .map(|key| {
                let mut new_parts = Vec::with_capacity(parts.len() + 1);
                new_parts.extend_from_slice(parts);
                new_parts.push(key);
                self.build_key(&new_parts)
            })
            .collect();

        debug!("Purging keys: {:?}", full_keys);

        let deleted: i64 = match redis::cmd("DEL")
            .arg(&full_keys)
            .query_async(&mut conn)
            .await
        {
            Ok(d) => {
                debug!("Redis DEL operation successful");
                d
            }
            Err(e) => {
                error!("Redis DEL operation failed: {}", e);
                return Err(e);
            }
        };

        info!("Purged {} keys", deleted);
        Ok(deleted)
    }

    /// Get all keys that start with the specified parts, returning only the parts after the provided prefix
    /// The owner prefix (/<owner_type>/<owner_id>) is automatically included and hidden from results
    pub async fn scan(&self, parts: &Vec<String>) -> RedisResult<Vec<String>> {
        let prefix = format!("{}/", self.build_key(parts));
        let pattern = format!("{}*", prefix);
        info!("Scanning for keys with pattern: {}", pattern);

        let mut conn = self.get_connection().await?;

        // Scan for keys matching the pattern
        let mut cursor = 0;
        let mut relative_keys = Vec::new();

        loop {
            trace!("SCAN iteration with cursor: {}", cursor);
            let (new_cursor, batch): (i64, Vec<String>) = match redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .query_async(&mut conn)
                .await
            {
                Ok(result) => {
                    trace!("SCAN successful");
                    result
                }
                Err(e) => {
                    error!("Redis SCAN operation failed: {}", e);
                    return Err(e);
                }
            };

            cursor = new_cursor;
            trace!("Next cursor: {}, batch size: {}", cursor, batch.len());

            // Extract relative parts (parts after the provided prefix)
            for key in batch {
                if key.starts_with(&prefix) {
                    let relative_key = key[prefix.len()..].to_string();
                    trace!("Found key: {} -> relative: {}", key, relative_key);
                    relative_keys.push(relative_key);
                }
            }

            if cursor == 0 {
                trace!("SCAN complete");
                break;
            }
        }

        info!(
            "Found {} keys matching pattern: {}",
            relative_keys.len(),
            pattern
        );
        Ok(relative_keys)
    }

    /// Dump all keys and values that start with the specified parts as JSON
    /// Returns a JSON object where keys are the relative paths (after the provided prefix)
    /// The owner prefix (/<owner_type>/<owner_id>) is automatically included and hidden from results
    pub async fn dump(&self, parts: &Vec<String>) -> RedisResult<JsonValue> {
        info!("Dumping keys with prefix: {:?}", parts);

        let keys = self.scan(parts).await?;

        info!("Found {} keys to dump", keys.len());

        if keys.is_empty() {
            debug!("No keys found, returning empty object");
            return Ok(JsonValue::Object(serde_json::Map::new()));
        }

        let full_keys: Vec<String> = keys
            .iter()
            .map(|key| {
                let mut new_parts = Vec::with_capacity(parts.len() + 1);
                new_parts.extend_from_slice(parts);
                new_parts.push(key.clone());
                self.build_key(&new_parts)
            })
            .collect();

        debug!("Getting values for keys: {:?}", full_keys);

        let mut conn = self.get_connection().await?;
        let values: Vec<Option<String>> = match redis::cmd("MGET")
            .arg(&full_keys)
            .query_async(&mut conn)
            .await
        {
            Ok(v) => {
                debug!("Redis MGET operation successful");
                v
            }
            Err(e) => {
                error!("Redis MGET operation failed: {}", e);
                return Err(e);
            }
        };

        let mut result = serde_json::Map::new();
        for (relative_key, maybe_value) in keys.into_iter().zip(values) {
            if let Some(value_str) = maybe_value {
                match string_to_value(&value_str) {
                    Ok(json_value) => {
                        trace!("Adding key to dump result: {}", relative_key);
                        result.insert(relative_key, json_value);
                    }
                    Err(e) => {
                        error!("Failed to deserialize JSON for key {}: {}", relative_key, e);
                        return Err(e);
                    }
                }
            }
        }

        info!("Successfully dumped {} key-value pairs", result.len());
        Ok(JsonValue::Object(result))
    }

    /// Restore data from a JSON dump
    /// The keys in the JSON are relative paths (after the provided prefix)
    /// These will be combined with the provided parts to form the full keys
    /// The owner prefix (/<owner_type>/<owner_id>) is automatically included
    pub async fn restore(&self, parts: &Vec<String>, json: JsonValue) -> RedisResult<i64> {
        info!("Restoring data with prefix: {:?}", parts);

        let prefix = self.build_key(parts);
        let mut conn = self.get_connection().await?;

        // If not an object, no keys to restore
        let JsonValue::Object(map) = json else {
            warn!("JSON is not an object, nothing to restore");
            return Ok(0);
        };

        // Build up (key, value) pairs for MSET
        // Redis expects them as a flat list: [key1, val1, key2, val2, ...]
        let mut args = Vec::with_capacity(map.len() * 2);
        for (relative_key, value) in map {
            let full_key = format!("{}/{}", prefix, relative_key);
            trace!("Preparing key for restore: {}", full_key);

            let value_str = match value_to_string(&value) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to serialize JSON for key {}: {}", full_key, e);
                    return Err(e);
                }
            };

            args.push(full_key);
            args.push(value_str);
        }

        // If there are no fields, we're done
        if args.is_empty() {
            debug!("No data to restore");
            return Ok(0);
        }

        debug!("Executing MSET for {} keys", args.len() / 2);

        // MSET all of them in one round trip
        if let Err(e) = redis::cmd("MSET")
            .arg(&args)
            .query_async::<()>(&mut conn)
            .await
        {
            error!("Redis MSET operation failed: {}", e);
            return Err(e);
        };
        info!("Successfully restored {} keys", args.len() / 2);

        // Each pair (full_key,value_str) is a single "set"
        Ok((args.len() as i64) / 2)
    }
}

#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub owner_type: String,
    pub owner_id: String,
}

// Thread-safe wrapper for the RedisRegistry
#[derive(Clone)]
pub struct AsyncRegistry {
    registry: Arc<RedisRegistry>,
}

impl AsyncRegistry {
    pub fn new(config: &RegistryConfig) -> Result<Self, RedisError> {
        info!("Creating AsyncRegistry with config: {:?}", config);

        let registry= RedisRegistry::new(&config.owner_type, &config.owner_id)?;
        debug!("RedisRegistry created successfully");

        Ok(AsyncRegistry {
            registry: Arc::new(registry),
        })
    }

    pub async fn set(&self, parts: &Vec<String>, value: JsonValue) -> RedisResult<()> {
        trace!("AsyncRegistry::set called with parts: {:?}", parts);
        self.registry.set(parts, value).await
    }

    pub async fn get(&self, parts: &Vec<String>) -> RedisResult<Option<JsonValue>> {
        trace!("AsyncRegistry::get called with parts: {:?}", parts);
        self.registry.get(parts).await
    }

    pub async fn delete(&self, parts: &Vec<String>) -> RedisResult<bool> {
        trace!("AsyncRegistry::delete called with parts: {:?}", parts);
        self.registry.delete(parts).await
    }

    pub async fn purge(&self, parts: &Vec<String>) -> RedisResult<i64> {
        trace!("AsyncRegistry::purge called with parts: {:?}", parts);
        self.registry.purge(parts).await
    }

    pub async fn scan(&self, parts: &Vec<String>) -> RedisResult<Vec<String>> {
        trace!("AsyncRegistry::scan called with parts: {:?}", parts);
        self.registry.scan(parts).await
    }

    pub async fn dump(&self, parts: &Vec<String>) -> RedisResult<JsonValue> {
        trace!("AsyncRegistry::dump called with parts: {:?}", parts);
        self.registry.dump(parts).await
    }

    pub async fn restore(&self, parts: &Vec<String>, json: JsonValue) -> RedisResult<i64> {
        trace!("AsyncRegistry::restore called with parts: {:?}", parts);
        self.registry.restore(parts, json).await
    }
}
