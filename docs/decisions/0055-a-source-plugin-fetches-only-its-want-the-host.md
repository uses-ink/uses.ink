## 0055 A source plugin fetches only its want; the host checks what it returns

Accepted. A source plugin chooses how much to fetch, not what. The host binds `git.fetch` to the
call's want (locator, revision and grant), lets `git.read` take only a clone that call fetched,
and includes the grant in the clone's cache key. A snapshot the plugin returns is accepted only
when it is of the commit the host read and names only files the host stored. The fetch is cut at
the call's wall-clock deadline, since fuel and epochs do not meter host calls. A source's terms
carry the runtime's tick or are refused. The `.cwasm` cache is sealed to the component's digest
and the engine, and is trusted as the builder's own code, not as input.

**Why:** with `network = local`, a plugin that could fetch any locator could publish a private
repository on the builder's disk. **Reversal:** a source that must fetch a repository it finds
while reading (submodules) needs a way for the want to name more than one repository.
