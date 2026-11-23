# Test a handoff in ChatGPT desktop

These two authored development cases use invented source documents. The Pavo
case changes an indexed source after an observation has been folded. The
Palisade case splits two incompatible observations across authors. The JSON
files include the expected answers and silent controls for the person grading
the exercise. Give the client the prompts below without the JSON answer keys.

Use a local development build and a new directory. The loader writes three
small source files and a separate store. It prints the root, store, workspace
ID and two task revisions.

```sh
python3 examples/task-handoff/prepare.py --bin /absolute/path/to/foundry \
  --out /absolute/new/demo
```

In ChatGPT desktop, add a dedicated STDIO MCP server for the exercise in
Settings → MCP servers. Use the installed Foundry executable as the command.
Supply each of the following as a separate argument, replacing the two paths
with the loader's output.

```text
--store
/absolute/new/demo/store
mcp
--root
/absolute/new/demo/source
--experimental-task-context
```

Restart the server and open the project chat. Use the exercise server's
`status` to read the workspace ID. Each prompt uses the same store the loader
closed. A second STDIO owner of that store must wait for the first one to exit.

After updating the executable, check the mounted `task_context` description
after the first `status` call. It should show command examples for both
`inspect` and `peek`. If the chat still shows the older description, wait
for the catalog to refresh and run the exercise in a fresh project chat.

For a user-level synthetic fallback that several desktop clients can discover,
prepare a separate seed and save the loader's JSON output as `manifest.json`
in that seed directory. Keep the seed store closed. Configure the STDIO
command as `python3` and pass the absolute path to `serve-demo.py` followed
by `--bin /absolute/path/to/foundry --demo /absolute/new/seed`. Limit the
user-level catalog with `enabled_tools = ["status", "search", "context",
"retrieve", "references"]`. Each connection opens a temporary copy of the
small synthetic store and removes its copy on orderly shutdown. This preserves
the synthetic source identity across connections. Use the project-scoped
exercise server above for task operations.

- **Pavo.** Resume task `pavo-slip` at revision 7. Which seal, layout and
  station should the slip use, and which decision is still open? Recover the
  original observation, check its source link and cite the current source.
- **Palisade.** Resume `palisade-press-handoff` at revision 11. Can you
  determine the cover stock for the production batch? Cite both original
  reports. Run `review` once for each proposed stock using the production
  assertion key from the task, then try the same stock under the proofing key.

Record the actual tool calls, source citations and unresolved items before
opening the answer keys. `review` compares the supplied structured assertion
with stored assertions. It does not choose which author is right.
