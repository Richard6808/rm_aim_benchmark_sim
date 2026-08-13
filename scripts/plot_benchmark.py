#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path

import matplotlib.pyplot as plt
import pandas as pd


def save_line(df, x, y, title, ylabel, output):
    clean = df[[x, y]].dropna().sort_values(x)
    if clean.empty:
        return
    fig, ax = plt.subplots(figsize=(8, 5))
    ax.plot(clean[x], clean[y], marker="o")
    ax.set_title(title)
    ax.set_xlabel("Target RPM")
    ax.set_ylabel(ylabel)
    ax.grid(True, alpha=0.25)
    fig.tight_layout()
    fig.savefig(output, dpi=160)
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("run_dir", type=Path)
    args = parser.parse_args()
    run = args.run_dir

    rpm = pd.read_csv(run / "rpm_degradation.csv")
    save_line(rpm, "rpm", "hit_rate_pct", "Hit rate vs target RPM", "Hit rate (%)", run / "hit_rate_vs_rpm.png")
    save_line(rpm, "rpm", "mean_dps", "DPS vs target RPM", "Mean DPS", run / "dps_vs_rpm.png")
    save_line(
        rpm,
        "rpm",
        "kill_success_rate_pct",
        "Kill success rate vs target RPM",
        "Kill success rate (%)",
        run / "kill_success_vs_rpm.png",
    )
    save_line(
        rpm,
        "rpm",
        "mean_kill_time_s",
        "Mean kill time vs target RPM (successful kills only)",
        "Mean kill time (s)",
        run / "kill_time_vs_rpm.png",
    )
    save_line(
        rpm,
        "rpm",
        "hit_rate_degradation_vs_0rpm_pct",
        "Hit-rate degradation vs 0 RPM",
        "Degradation (%)",
        run / "hit_rate_degradation_vs_rpm.png",
    )
    save_line(
        rpm,
        "rpm",
        "dps_degradation_vs_0rpm_pct",
        "DPS degradation vs 0 RPM",
        "Degradation (%)",
        run / "dps_degradation_vs_rpm.png",
    )

    cond = pd.read_csv(run / "conditions.csv")
    for distance in sorted(cond["distance_m"].unique()):
        part = cond[cond["distance_m"] == distance]
        fig, ax = plt.subplots(figsize=(8, 5))
        for speed in sorted(part["translation_speed_mps"].unique()):
            line = part[part["translation_speed_mps"] == speed].sort_values("rpm")
            ax.plot(line["rpm"], line["mean_hit_rate_pct"], marker="o", label=f"{speed:g} m/s")
        ax.set_title(f"Hit rate vs RPM at {distance:g} m")
        ax.set_xlabel("Target RPM")
        ax.set_ylabel("Mean hit rate (%)")
        ax.grid(True, alpha=0.25)
        ax.legend(title="Translation speed")
        fig.tight_layout()
        fig.savefig(run / f"hit_rate_{distance:g}m.png", dpi=160)
        plt.close(fig)

    print(f"plots written to {run}")


if __name__ == "__main__":
    main()
