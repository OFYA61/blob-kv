**Goal**: Create a CLI tool to query the DB over a network connection.
### Protocol

Request formats:
* `SET key value`
* `GET key`
* `DEL key`
Parsing rules:
* Spaces between commands allowed
* New lines between commands **not** allowed
* Values containing spaces must be wrapped inside `""`
* An absent `value` will be treated as an empty value
* Unknown commands will return an error
* Excess arguments will return an error
* Invalid UTF-8 will return an error
* Oversized requests will return an error
### Implement: Server

The server must:
- [ ] Bind to a configurable local address and port
- [ ] Accepts client connections
- [ ] Reads requests
- [ ] Parses complete commands
- [ ] Invokes the engine
- [ ] Formats the response
- [ ] Write the response to the client
- [ ] Handles disconnects and I/O errors without crashing the whole server

Simple thread-per-connection design to keep the scope small. No async runtime or event loops as of yet.

##### Request format

| Field          | Purpose                                   | Size     | Notes                  |
| -------------- | ----------------------------------------- | -------- | ---------------------- |
| Version        | Version of the request format             | 1 byte   |                        |
| Payload length | The size of the remaining fields in bytes | 4 bytes  |                        |
| OpCode         | The operation that is being done          | 1 byte   |                        |
| Key length     | The length of the key                     | 4 bytes  |                        |
| Key            | The contents of the key                   | Variable |                        |
| Value length   | The length of the value                   | 4 bytes  | Only present for `SET` |
| Value          | The contents of the value                 | Variable | Only present for `SET` |
##### OpCode

| Code | Value  |
| ---- | ------ |
| 1    | SET    |
| 2    | GET    |
| 3    | DELETE |

##### Response format

| Field          | Purpose                              | Size     |
| -------------- | ------------------------------------ | -------- |
| Version        | Version of the response format       | 1 byte   |
| Status         | Indicates success or failure         | 1 byte   |
| Payload length | Number of bytes following the header | 4 bytes  |
| Payload        | The contents of the payload          | Variable |

##### Status codes


| Code | Meaning                  |
| ---- | ------------------------ |
| 0x00 | OK                       |
| 0x01 | Key not found            |
| 0x02 | Invalid request          |
| 0x03 | Unsupported operation    |
| 0x04 | Internal server error    |
| 0x05 | Value too large          |
| 0x06 | Invalid protocol version |

### Implement: CLI Client

- [ ] Connect to the configured host and port
- [ ] Accept commands from a terminal
- [ ] Send request
- [ ] Read complete responses
- [ ] Display results in a human-readable format
- [ ] Report connection failures
- [ ] Exit cleanly when requested

### Documentation

Document how to run it in the `README.md` file of the repo.