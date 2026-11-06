"use client";

import { useState, type FormEvent, type ReactNode } from "react";

import { api, ApiError } from "@/lib/api";

import styles from "./EmailForm.module.css";

type Props = {
  action: "checkout" | "trial" | "recover";
  label: string;
  busyLabel: string;
  // Shown once the request succeeds. Checkout has none: it leaves for the payment page.
  done?: ReactNode;
};

async function submit(action: Props["action"], email: string) {
  if (action !== "checkout") return void (await api(`/${action}`, { email }));
  const { url } = await api<{ url: string }>("/checkout", { email });
  // Only follow web URLs, whatever the server sent.
  if (!/^https?:$/.test(new URL(url).protocol)) throw new ApiError("The checkout link was invalid. Try again.", 0);
  location.assign(url);
}

export function EmailForm({ action, label, busyLabel, done }: Props) {
  const [state, setState] = useState<"idle" | "busy" | "done">("idle");
  const [error, setError] = useState("");

  async function onSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const email = String(new FormData(e.currentTarget).get("email") ?? "").trim();
    setState("busy");
    setError("");
    try {
      await submit(action, email);
      if (done) setState("done");
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Something went wrong. Try again in a moment.");
      setState("idle");
    }
  }

  if (state === "done") return <div className={styles.done} role="status">{done}</div>;

  return (
    <form onSubmit={onSubmit} className={styles.form}>
      <label htmlFor="email">Email address</label>
      <div className={styles.row}>
        <input
          id="email"
          name="email"
          type="email"
          className="field"
          autoComplete="email"
          maxLength={254}
          placeholder="you@example.com"
          required
          disabled={state === "busy"}
        />
        <button className="btn btn-primary" disabled={state === "busy"}>
          {state === "busy" ? busyLabel : label}
        </button>
      </div>
      <p className={styles.error} role="alert">
        {error}
      </p>
    </form>
  );
}
