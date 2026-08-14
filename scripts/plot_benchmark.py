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
    ax.set_xlabel("Target angular speed (rad/s)")
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

    angular = pd.read_csv(run / "angular_speed_degradation.csv")
    save_line(
        angular,
        "angular_speed_rad_s",
        "hit_rate_pct",
        "Hit rate vs target angular speed",
        "Hit rate (%)",
        run / "hit_rate_vs_angular_speed.png",
    )
    save_line(
        angular,
        "angular_speed_rad_s",
        "mean_dps",
        "DPS vs target angular speed",
        "Mean DPS",
        run / "dps_vs_angular_speed.png",
    )
    save_line(
        angular,
        "angular_speed_rad_s",
        "kill_success_rate_pct",
        "Kill success rate vs target angular speed",
        "Kill success rate (%)",
        run / "kill_success_vs_angular_speed.png",
    )
    save_line(
        angular,
        "angular_speed_rad_s",
        "mean_kill_time_s",
        "Mean kill time vs target angular speed (successful kills only)",
        "Mean kill time (s)",
        run / "kill_time_vs_angular_speed.png",
    )
    save_line(
        angular,
        "angular_speed_rad_s",
        "hit_rate_degradation_vs_stationary_pct",
        "Hit-rate degradation vs stationary target",
        "Degradation (%)",
        run / "hit_rate_degradation_vs_angular_speed.png",
    )
    save_line(
        angular,
        "angular_speed_rad_s",
        "dps_degradation_vs_stationary_pct",
        "DPS degradation vs stationary target",
        "Degradation (%)",
        run / "dps_degradation_vs_angular_speed.png",
    )

    cond = pd.read_csv(run / "conditions.csv")
    for distance in sorted(cond["distance_m"].unique()):
        part = cond[cond["distance_m"] == distance]
        fig, ax = plt.subplots(figsize=(8, 5))
        for speed in sorted(part["translation_speed_mps"].unique()):
            line = part[part["translation_speed_mps"] == speed].sort_values(
                "angular_speed_rad_s"
            )
            ax.plot(
                line["angular_speed_rad_s"],
                line["mean_hit_rate_pct"],
                marker="o",
                label=f"{speed:g} m/s",
            )
        ax.set_title(f"Hit rate vs angular speed at {distance:g} m")
        ax.set_xlabel("Target angular speed (rad/s)")
        ax.set_ylabel("Mean hit rate (%)")
        ax.grid(True, alpha=0.25)
        ax.legend(title="Translation speed")
        fig.tight_layout()
        fig.savefig(run / f"hit_rate_{distance:g}m.png", dpi=160)
        plt.close(fig)

    print(f"plots written to {run}")


if __name__ == "__main__":
    main()
