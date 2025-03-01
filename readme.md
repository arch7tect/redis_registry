# Redis Registry API

A simple, hierarchical key-value store built on top of Redis with a RESTful API interface. This service allows you to store, retrieve, and manage JSON values using path-based keys.

## Features

- Store, retrieve, update, and delete JSON values
- Hierarchical key organization with path-based access
- Batch operations (purge, dump, restore)
- Interactive Swagger UI documentation
- Comprehensive async logging with structured JSON output

## Installation

### Prerequisites

- Rust (edition 2024)
- Redis server

### Setup

1. Clone the repository
   ```
   git clone https://github.com/arch7tect/redis_registry.git
   cd redis_registry
   ```

2. Configure environment variables (create a `.env` file):
   ```
   REDIS_URL=redis://localhost:6379
   # Or use separate host/port:
   # REDIS_HOST=localhost
   # REDIS_PORT=6379
   
   # Define owner namespace for keys
   OWNER_TYPE=myapp
   OWNER_ID=instance1
   
   # Rocket server configuration
   ROCKET_PORT=8000
   
   # Logging configuration
   RUST_LOG=info
   LOG_DIR=logs
   ```

3. Build and run
   ```
   cargo build --release
   cargo run --release
   ```

## API Endpoints

All endpoints support using the `?path=` query parameter to specify key paths. Paths can be:
- Empty (root level operations)
- Simple keys (`?path=mykey`)
- Nested paths (`?path=users/profiles/admin`)

### Available Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/redis/get?path=...` | Get a JSON value by key path |
| POST | `/redis/set?path=...` | Set a JSON value for key path |
| DELETE | `/redis/delete?path=...` | Delete a key by path |
| POST | `/redis/purge?path=...` | Delete all keys with the specified prefix |
| GET | `/redis/scan?path=...` | List all keys with the specified prefix |
| GET | `/redis/dump?path=...` | Dump all keys and values with the specified prefix |
| POST | `/redis/restore?path=...` | Restore data from a JSON dump |

### Examples

#### Store a value

```
POST /redis/set?path=users/john
Content-Type: application/json

{
  "name": "John Doe",
  "email": "john@example.com",
  "role": "admin"
}
```

#### Retrieve a value

```
GET /redis/get?path=users/john
```

#### List all user keys

```
GET /redis/scan?path=users
```

## Swagger UI

The API includes an interactive Swagger UI for documentation and testing:

```
http://localhost:8000/swagger-ui/
```

## Key Organization

Keys are organized with the following structure:

```
/<owner_type>/<owner_id>/<user-defined-path>
```

The `owner_type` and `owner_id` are automatically prepended to all keys, allowing multiple applications or instances to share the same Redis instance safely.

## Internal Data Structure

All values are stored as JSON strings in Redis. The API handles serialization and deserialization transparently.

## Configuration Options

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `REDIS_URL` | Full Redis connection URL | - |
| `REDIS_HOST` | Redis server hostname (alternative to URL) | - |
| `REDIS_PORT` | Redis server port (alternative to URL) | 6379 |
| `OWNER_TYPE` | Namespace prefix (first level) | "default" |
| `OWNER_ID` | Instance identifier (second level) | "default" |
| `ROCKET_PORT` | HTTP server port | 8000 |
| `RUST_LOG` | Log level (trace, debug, info, warn, error) | "info" |
| `LOG_DIR` | Directory for log files | "logs" |

## Logging

The Redis Registry includes comprehensive logging capabilities:

### Log Outputs

- **Console**: Human-readable logs output to stdout
- **File**: JSON-formatted logs stored in rotating daily files

### Log Levels

- **ERROR**: Operational errors that require attention
- **WARN**: Potentially harmful situations but not critical
- **INFO**: General operational information (default)
- **DEBUG**: Detailed information for debugging
- **TRACE**: Highly detailed information for development

### Log File Structure

Logs are stored in the configured `LOG_DIR` with daily rotation:
```
logs/
  └── redis-registry.YYYY-MM-DD
```

### Log Configuration

You can configure logging using environment variables:

- `RUST_LOG`: Sets the log level (trace, debug, info, warn, error)
- `LOG_DIR`: Directory where log files will be stored

## Development

### Running Tests

```
cargo test
```

### Building Documentation

```
cargo doc --open
```

## License

[MIT License](LICENSE)