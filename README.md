# blitz-deploy

This is an **experimental** tool for quickly updating NixOS configurations. The assumption is that:

- You run CI to build your NixOS configuration.
- You push the resulting store path into a binary cache.
- Evaluating your NixOS configuration is slow.

If this is true, this tool might be useful for you. What it does for now:

- Fetch the current NixOS toplevel store path from Hercules CI.
- Fetch it from the configured binary caches.
- Deploy it.

## TODO

- Write a NixOS module to update periodically.
- Support Botanix?
