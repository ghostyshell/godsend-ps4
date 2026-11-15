import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import type { PadStatus } from "../types";
import { Btn, Chip } from "../ui";

// Control4Free's input message, from its docs/protocol.md:
//   {"method":"i","params":[pad,seq,buttons,lx,ly,rx,ry,l2,r2,fingers,...,historyCount,...]}
// Sticks and triggers are 0..255 with 128 centred; up and left are 0.
const BITS: Record<string, number> = {
  share: 0x0001,
  l3: 0x0002,
  r3: 0x0004,
  options: 0x0008,
  up: 0x0010,
  right: 0x0020,
  down: 0x0040,
  left: 0x0080,
  l2: 0x0100,
  r2: 0x0200,
  l1: 0x0400,
  r1: 0x0800,
  triangle: 0x1000,
  circle: 0x2000,
  cross: 0x4000,
  square: 0x8000,
  ps: 0x10000,
  touchpad: 0x100000,
};

interface PadState {
  buttons: number;
  lx: number;
  ly: number;
  rx: number;
  ry: number;
  l2: number;
  r2: number;
}

const NEUTRAL: PadState = { buttons: 0, lx: 128, ly: 128, rx: 128, ry: 128, l2: 0, r2: 0 };

export default function Controller({ snap, toast, reload }: PageProps) {
  const [status, setStatus] = useState<PadStatus | null>(snap.pad);
  const [state, setState] = useState<"idle" | "connecting" | "live" | "error">("idle");
  const [error, setError] = useState<string | null>(null);
  const [padIndex, setPadIndex] = useState(0);

  const pcRef = useRef<RTCPeerConnection | null>(null);
  const inputRef = useRef<RTCDataChannel | null>(null);
  const controlRef = useRef<RTCDataChannel | null>(null);
  const stateRef = useRef<PadState>({ ...NEUTRAL });
  const seqRef = useRef(0);
  const timerRef = useRef<number | null>(null);

  const active = snap.statuses.find((s) => s.host_id === snap.active) ?? null;

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await api.padStatus());
    } catch (err) {
      setStatus({ ok: false, version: null, protocol: null, pads: null, message: String(err) });
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  const teardown = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearInterval(timerRef.current);
      timerRef.current = null;
    }
    inputRef.current?.close();
    controlRef.current?.close();
    pcRef.current?.close();
    inputRef.current = null;
    controlRef.current = null;
    pcRef.current = null;
    stateRef.current = { ...NEUTRAL };
  }, []);

  const stop = useCallback(() => {
    teardown();
    setState("idle");
  }, [teardown]);

  useEffect(() => stop, [stop]);

  const send = useCallback((method: string, params: unknown[], id?: number) => {
    const channel = method === "i" || method === "m" || method === "e" ? inputRef.current : controlRef.current;
    if (channel?.readyState !== "open") return;
    channel.send(JSON.stringify(id === undefined ? { method, params } : { id, method, params }));
  }, []);

  const start = useCallback(async () => {
    teardown();
    setError(null);
    setState("connecting");
    try {
      const pc = new RTCPeerConnection({ iceServers: [] });
      pcRef.current = pc;
      const control = pc.createDataChannel("control", { negotiated: true, id: 0, ordered: true });
      const input = pc.createDataChannel("input", {
        negotiated: true,
        id: 1,
        ordered: false,
        maxRetransmits: 0,
      });
      controlRef.current = control;
      inputRef.current = input;

      pc.onconnectionstatechange = () => {
        if (pc.connectionState === "failed" || pc.connectionState === "disconnected") {
          teardown();
          setError("the console dropped the WebRTC link");
          setState("error");
        }
      };

      control.onopen = () => {
        control.send(JSON.stringify({ id: 7, method: "claim", params: [padIndex] }));
      };

      input.onopen = () => {
        setState("live");
        // Send at 60 Hz: the payload ignores stale sequences, so a steady
        // stream also repairs a lost button release.
        timerRef.current = window.setInterval(() => {
          const s = stateRef.current;
          seqRef.current += 1;
          send("i", [
            padIndex,
            seqRef.current,
            s.buttons,
            s.lx,
            s.ly,
            s.rx,
            s.ry,
            s.l2,
            s.r2,
            0,
            0,
          ]);
        }, 1000 / 60);
      };

      control.onmessage = (event) => {
        try {
          const reply = JSON.parse(String(event.data));
          if (reply?.error) setError(String(reply.error));
        } catch {
          // Replies are informational; ignore anything unparseable.
        }
      };

      const offer = await pc.createOffer();
      await pc.setLocalDescription(offer);
      if (!pc.localDescription) throw new Error("the browser produced no local description");
      const answer = await api.padOffer(pc.localDescription.sdp);
      await pc.setRemoteDescription({ type: "answer", sdp: answer });
    } catch (err) {
      teardown();
      setError(String(err));
      setState("error");
    }
  }, [padIndex, send, teardown]);

  const press = useCallback((name: string, down: boolean) => {
    stateRef.current = {
      ...stateRef.current,
      buttons: down
        ? stateRef.current.buttons | (BITS[name] ?? 0)
        : stateRef.current.buttons & ~(BITS[name] ?? 0),
    };
  }, []);

  const setTrigger = useCallback((name: "l2" | "r2", value: number) => {
    stateRef.current = { ...stateRef.current, [name]: value };
  }, []);

  useEffect(() => {
    if (state !== "live") return;
    const map: Record<string, string> = {
      ArrowUp: "up",
      ArrowDown: "down",
      ArrowLeft: "left",
      ArrowRight: "right",
      j: "cross",
      k: "circle",
      l: "square",
      i: "triangle",
      q: "l1",
      e: "r1",
      Shift: "l3",
      Control: "r3",
      Enter: "options",
      Backspace: "share",
    };
    const down = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target && ["INPUT", "TEXTAREA"].includes(target.tagName)) return;
      const name = map[event.key] ?? map[event.key.toLowerCase()];
      if (!name) return;
      event.preventDefault();
      press(name, true);
    };
    const up = (event: KeyboardEvent) => {
      const name = map[event.key] ?? map[event.key.toLowerCase()];
      if (!name) return;
      press(name, false);
    };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
    };
  }, [press, state]);

  const installPad = async () => {
    try {
      await api.homebrewInstall("control4free");
      toast("installing Control4Free; the console shows the progress");
      reload();
    } catch (err) {
      toast(`install failed: ${String(err)}`);
    }
  };

  const live = state === "live";
  const statusLine = useMemo(() => {
    if (!status) return "checking the pad service";
    if (!status.ok) return status.message || "the pad service is not running";
    return `Control4Free ${status.version ?? ""} - protocol ${status.protocol ?? "?"} - ${
      status.pads ?? 1
    } pads`;
  }, [status]);

  return (
    <div className="page">
      <div className="panel">
        <div className="panel-head">
          <h2>Virtual DualShock 4</h2>
          <div className="spacer" />
          <Chip tone={status?.ok ? "good" : "warn"} live={status?.ok}>
            {status?.ok ? "service ready" : "service offline"}
          </Chip>
          {live ? (
            <Btn small variant="danger" onClick={stop}>
              Disconnect
            </Btn>
          ) : (
            <Btn small variant="primary" disabled={!active || state === "connecting"} onClick={() => void start()}>
              {state === "connecting" ? "Connecting..." : "Connect"}
            </Btn>
          )}
        </div>
        <p style={{ marginTop: 0, color: "var(--text-dim)", fontSize: 13 }}>{statusLine}</p>
        {error ? <p style={{ color: "var(--bad)", fontSize: 13 }}>{error}</p> : null}
        {!status?.ok ? (
          <div className="quick">
            <Btn variant="primary" onClick={() => void installPad()}>
              Install Control4Free
            </Btn>
            <Btn onClick={() => void refreshStatus()}>Re-check</Btn>
          </div>
        ) : null}
        {status?.ok && !live ? (
          <div className="field" style={{ maxWidth: 220 }}>
            <label>Controller slot</label>
            <select value={padIndex} onChange={(e) => setPadIndex(Number(e.target.value))}>
              {Array.from({ length: status.pads ?? 1 }, (_, i) => (
                <option key={i} value={i}>
                  Player {i + 1}
                </option>
              ))}
            </select>
          </div>
        ) : null}
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>Pad</h2>
          <div className="spacer" />
          <span className="mono" style={{ fontSize: 12, color: "var(--text-faint)" }}>
            {live ? "live" : "not connected"}
          </span>
        </div>

        <div className={`pad ${live ? "live" : ""}`} aria-label="Virtual controller">
          <div className="pad-shoulders">
            <Trigger label="L2" onChange={(v) => setTrigger("l2", v)} />
            <PadButton name="l1" label="L1" press={press} />
            <PadButton name="r1" label="R1" press={press} />
            <Trigger label="R2" onChange={(v) => setTrigger("r2", v)} />
          </div>

          <div className="pad-main">
            <div className="dpad">
              <PadButton className="du" name="up" label="Up" press={press} />
              <PadButton className="dl" name="left" label="Left" press={press} />
              <PadButton className="dr" name="right" label="Right" press={press} />
              <PadButton className="dd" name="down" label="Down" press={press} />
            </div>

            <div className="face">
              <PadButton className="ft" name="triangle" label="Tri" press={press} />
              <PadButton className="fs" name="square" label="Sq" press={press} />
              <PadButton className="fc" name="circle" label="Circ" press={press} />
              <PadButton className="fx" name="cross" label="X" press={press} />
            </div>
          </div>

          <div className="pad-center">
            <PadButton className="small" name="share" label="Share" press={press} />
            <PadButton className="small" name="ps" label="PS" press={press} />
            <PadButton className="small" name="touchpad" label="Touchpad" press={press} />
            <PadButton className="small" name="options" label="Options" press={press} />
          </div>

          <div className="pad-sticks">
            <div>
              <div className="stick-label">L3</div>
              <Stick label="left stick" side="l" onChange={(x, y) => {
                stateRef.current = { ...stateRef.current, lx: x, ly: y };
              }} />
            </div>
            <div>
              <div className="stick-label">R3</div>
              <Stick label="right stick" side="r" onChange={(x, y) => {
                stateRef.current = { ...stateRef.current, rx: x, ry: y };
              }} />
            </div>
          </div>
        </div>

        <p className="mono" style={{ fontSize: 11.5, color: "var(--text-faint)" }}>
          keyboard: arrows d-pad, J cross, K circle, L square, I triangle, Q/E L1/R1, Shift/Ctrl
          L3/R3, Enter options, Backspace share
        </p>
      </div>
    </div>
  );
}

function PadButton({
  name,
  label,
  press,
  className,
}: {
  name: string;
  label: string;
  press: (name: string, down: boolean) => void;
  className?: string;
}) {
  return (
    <button
      className={`pad-btn ${className ?? ""}`}
      onPointerDown={(e) => {
        e.preventDefault();
        press(name, true);
      }}
      onPointerUp={() => press(name, false)}
      onPointerLeave={() => press(name, false)}
      onPointerCancel={() => press(name, false)}
      aria-label={label}
    >
      {label}
    </button>
  );
}

function Trigger({ label, onChange }: { label: string; onChange: (value: number) => void }) {
  return (
    <label className="pad-trigger">
      <span>{label}</span>
      <input
        type="range"
        min={0}
        max={255}
        defaultValue={0}
        onChange={(e) => onChange(Number(e.target.value))}
        aria-label={label}
      />
    </label>
  );
}

function Stick({
  label,
  side,
  onChange,
}: {
  label: string;
  side: "l" | "r";
  onChange: (x: number, y: number) => void;
}) {
  const ref = useRef<HTMLDivElement | null>(null);
  const dragging = useRef(false);

  const move = (event: React.PointerEvent) => {
    const box = ref.current?.getBoundingClientRect();
    if (!box) return;
    const cx = box.left + box.width / 2;
    const cy = box.top + box.height / 2;
    const dx = Math.max(-1, Math.min(1, (event.clientX - cx) / (box.width / 2)));
    const dy = Math.max(-1, Math.min(1, (event.clientY - cy) / (box.height / 2)));
    // Screen y grows down; the protocol wants 0 at the top.
    onChange(Math.round(128 + dx * 127), Math.round(128 + dy * 127));
  };

  return (
    <div
      ref={ref}
      className={`stick ${side}`}
      role="slider"
      aria-label={`${label} stick`}
      aria-valuemin={-1}
      aria-valuemax={1}
      aria-valuenow={0}
      onPointerDown={(e) => {
        dragging.current = true;
        e.currentTarget.setPointerCapture(e.pointerId);
        move(e);
      }}
      onPointerMove={(e) => {
        if (dragging.current) move(e);
      }}
      onPointerUp={() => {
        dragging.current = false;
        onChange(128, 128);
      }}
      onPointerCancel={() => {
        dragging.current = false;
        onChange(128, 128);
      }}
    >
      <span className="nub" />
    </div>
  );
}
