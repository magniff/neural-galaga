#!/usr/bin/env python3
"""Plot training_log.csv: avg_reward over updates with a smoothed trend line."""

import sys
import pandas as pd
import matplotlib.pyplot as plt

path = sys.argv[1] if len(sys.argv) > 1 else "checkpoints/cheats/training_log.csv"
df = pd.read_csv(path)

window = max(1, len(df) // 100)
smoothed = df["avg_reward"].rolling(window, min_periods=1).mean()

fig, ax = plt.subplots(figsize=(12, 5))
ax.plot(df["update"], df["avg_reward"], alpha=0.3, label="raw")
ax.plot(df["update"], smoothed, linewidth=2, label=f"smoothed (window={window})")
ax.set_xlabel("Update")
ax.set_ylabel("Avg Episode Reward")
ax.set_title("Cheats Agent Training")
ax.legend()
ax.grid(True, alpha=0.3)
plt.tight_layout()
out = path.rsplit(".", 1)[0] + ".png"
plt.savefig(out, dpi=150)
print(f"saved to {out}")
