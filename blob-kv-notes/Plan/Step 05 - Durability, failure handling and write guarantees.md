**Goal**: Define what a successful write means and make the engine consistent with that promise.

### Durability contract

> A successful mutation response is sent only after the record has been appended successfully and the configured durability synchronization has completed successfully.

### Recovery on `SET` operation fail

1. Application failed to construct the record
	- Abort `SET` operation
	- Do not write anything on disk
	- Do not modify the in-memory index
	- Return an error to the client
2. Operating system fails to write through file I/O
	- Handle partial writes by retrying until all bytes are written or an unrecoverable error occurs.
	- If the write fails 100%, mark the database as requiring recovery
	- Do not update the in-memory index
	- Do not report success
	- On restart, scan the log and discard any incomplete trailing record
3. In-memory indexing fails
	* The record is already durable on the disk
	* Try rebuilt the memory index from the log
	* If unsuccessful, remove the record from the log
4. Sending response back to client fails
	 * Treat it as if the `SET` operation succeeded
### Testing

- [ ] Key or value exceeds the configured limit
- [ ] Record encoding fails
- [ ] The log file cannot be opened
- [ ] A write fails before writing any bytes
- [ ] A write fails after writing part of a record
- [ ] File synchronization fails