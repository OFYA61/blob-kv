**Goal**: Establish how a record is represented, establish a way to convert between an in-memory record and it's binary representation.
### Data Model

- `key`: UTF-8 string, case sensitive, empty values **NOT** allowed, spaces **NOT** allowed
- `value`: UTF-8 string, empty values allowed, spaces allowed
### Operations

| Operation       | Behaviour                                                   |
| --------------- | ----------------------------------------------------------- |
| `SET key value` | Upsert a value                                              |
| `GET key`       | Return the current value or indicate that the key is absent |
| `DEL key`       | Remove a key and return whether it existed                  |
### Record Model

Maximum record model size is 1,049,618 bytes, around 1 MiB.

| Field          | Purpose                                                                                                                              | Size                            |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------- |
| Record length  | The size of the encoded record                                                                                                       | 4 bytes                         |
| Format version | Identifies the format of the record                                                                                                  | 1 byte                          |
| Operation type | `SET` or `DEL`                                                                                                                       | 1 byte                          |
| Key length     | Number of key bytes                                                                                                                  | 4 bytes                         |
| Value length   | Number of value bytes                                                                                                                | 4 bytes                         |
| Key bytes      | Stores the key in UTF-8                                                                                                              | Variable (max. 1,024 bytes)     |
| Value bytes    | Stores the value in UTF-8                                                                                                            | Variable (max. 1,048,576 bytes) |
| Checksum       | Used to detect corruption (calculated using every field except the `Checksum` field itself). Use `CRC32C` as the checksum algorithm. | 4 bytes                         |
### Implement

- [x] Encode a valid logical record into bytes
- [x] Decode bytes into a valid logic record
- [x] Reject records that violate the format
- [x] Calculate and verify the checksum

### Testing

- [x] A short key and short value
- [x] An empty value
- [x] A long value within the configured limit