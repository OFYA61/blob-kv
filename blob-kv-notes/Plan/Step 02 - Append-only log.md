**Goal**: Store encoded records in a file, read them back sequentially, and establish the persistence layer.

### Responsibilities of the log layer

- Own the log file
- Append and encode record
- Track the location of a record within the file
- Read records sequentially for recovery
- Report file I/O failures
### Record position

The position of a record is the byte offset at which it begins.
### Opening the log file

- If file does note exist, create it
- If file exists, open it without overriding it's contents
- Rejects paths that point towards an unsuitable file
- Lock the file using kernel-managed file locks to prevent 2 instances writing to it
### Implement: appending

Operations that accept a valid logical record, encodes it, and append it to the log:
- [ ] Validate the record
- [ ] Encode the record
- [ ] Determine the intended starting offset
- [ ] Write the complete encoded record
- [ ] Return the record position if the write succeeds
- [ ] Handle append failures

### Implement: sequential scanning

Reader that walks through the file from the beginning:
- [ ] Determine how much data is needed to read the header
- [ ] Validate the declared record length against the limits
- [ ] Read the remaining record bytes
- [ ] Decode and validate the record
- [ ] Report the records's starting offset
- [ ] Move to the next record

### Testing

- [ ] Append one record and scan it
- [ ] Append several records and verify their order
- [ ] Close the log and reopen it
- [ ] Scan an empty file
- [ ] Append the records of different sizes
- [ ] Attempt to open a missing directory or otherwise invalid path
- [ ] Test a partial write or simulated I/O error
- [ ] Confirm that reopening an existing file does not erase previous data