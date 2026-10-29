**Goal**: Turn the log into a usable database by implementing the index and the 3 core operations.

### In memory state

An index mapping keys to the latest record's position in the log. The index **must** contain only currently active keys. If a key has been deleted, it should not remain as a normal active entry.

### Implement: `SET` operation

- [ ] Validate the key and value against the limits
- [ ] Construct a set record
- [ ] Append the record to the log
- [ ] Update the index to point to the new record
- [ ] Return success or an error

### Implement: `GET` operation

- [ ] Search the in-memory index for the key
- [ ] If an entry exists, read the corresponding record from the log and return it's value
- [ ] If the key is absent, return the missing-key result

#### Implement: `DEL` operation

- [ ] Insert deletion record to the log
- [ ] Remove key index from memory
- [ ] If key did not exist, return success indicating no action was taken
### Testing

- [ ] `GET` a key that exists
- [ ] `GET` a key that does not exist
- [ ] `GET` a key after being overwritten
- [ ] `GET` empty values
- [ ] `GET` uni-code values
- [ ] Handle a `GET` record failure
- [ ] Handle missing key
- [ ] `DEL` a key
- [ ] `DEL` a non-existent key