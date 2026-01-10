# Repo-local AI Orchestrator state

This folder holds **repo-local orchestration state** used by the AI swarm.

Tracked:\n+ - `roadmap_index.json`: the shared index of work orders (dependencies + status)\n+ - `work_orders/`: work order specs (machine-readable)\n+\n+Gitignored:\n+ - `artifacts/`: logs, traces, screenshots, build outputs\n+ - `cache/`: temporary cache\n+
If you want to reset local artifacts:\n+\n+```powershell\n+Remove-Item -Recurse -Force .ai\\artifacts, .ai\\cache\n+```\n+

