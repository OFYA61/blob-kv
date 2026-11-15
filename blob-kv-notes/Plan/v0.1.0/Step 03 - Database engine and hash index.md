**Goal**: Turn the log into a usable database by implementing the index and the 3 core operations.

### In memory state

An index mapping keys to the latest record's position in the log. The index **must** contain only currently active keys. If a key has been deleted, it should not remain as a normal active entry.

### Implement: `SET` operation

- [x] Validate the key and value against the limits
- [x] Construct a set record
- [x] Append the record to the log
- [x] Update the index to point to the new record
- [x] Return success or an error

### Implement: `GET` operation

- [x] Search the in-memory index for the key
- [x] If an entry exists, read the corresponding record from the log and return it's value
- [x] If the key is absent, return the missing-key result

#### Implement: `DEL` operation

- [x] Insert deletion record to the log
- [x] Remove key index from memory
- [x] If key did not exist, return success indicating no action was taken
### Testing

- [x] `GET` a key that exists
- [x] `GET` a key that does not exist
- [x] `GET` a key after being overwritten
- [x] `GET` empty values
- [x] `GET` uni-code values
- [x] Handle a `GET` record failure
- [x] Handle missing key
- [x] `DEL` a key
- [x] `DEL` a non-existent key