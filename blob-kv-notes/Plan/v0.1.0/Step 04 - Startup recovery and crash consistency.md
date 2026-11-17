**Goal**: Reconstruct the state from disk and handle incomplete or corrupt records predictably.

### Implement: Index reconstruction

On startup:
- [x] Open the log
- [x] Initialize the empty index
- [x] Scan records from the beginning
- [x] For each set record, insert or replace its keys index entry
- [x] For each deletion record, remove the keys index entry
- [x] Finish with the index representing the latest state of the log

Use the record scanner and decoder from [[Step 01 - Codec and on-disk format]] and [[Step 02 - Append-only log]].

### Implement: Handle incomplete and corrupt logs

For corruption in the middle of the log:
- [x] Report the location of the problem
- [x] Fail startup rather to avoid using potentially valid data after corrupted section
- [x] Explain the failure clearly enough that the user can investigate it

Automatic recovery not part of the scope initially. The only exception is a trailing invalid record. Recover from that by removing it from the log file.
### Testing

Create the test suite that creates the database, performs operations, closes it, and opens it again:
- [x] Set a key, close the database, reopen it, and retrieve the value
- [x] Replace a keys value several times and verify the latest value is being read
- [x] Delete a key, restart, and verify that it remains absent
- [x] Delete a key and then set it again
- [x] Store several keys and ensure their states are reconstructed
- [x] Open an empty database
- [x] Recover from a valid log containing many records
- [x] Reject a log with a corrupt record in the middle
      Verify that after fixing the corruption, the database can recover from the file