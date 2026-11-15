import { useState } from "react";
import type { PageProps } from "../App";
import { api, pickers } from "../api";
import type { ConvertInfo } from "../types";
import { Btn, Chip, Empty, fmtBytes } from "../ui";

export default function Convert({ snap, toast, reload }: PageProps) {
  const [info, setInfo] = useState<ConvertInfo | null>(null);
  const [chunkMb, setChunkMb] = useState(Math.round(snap.config.split_bytes / (1024 * 1024)) || 4095);
  const [busy, setBusy] = useState(false);

  async function inspect(path: string) {
    try {
      setInfo(await api.convertInspect(path));
    } catch (err) {
      toast(`inspect failed: ${String(err)}`);
    }
  }

  async function pickFile() {
    const file = await pickers.pickFile("Choose a PKG, payload, ZIP or split part", [
      "pkg",
      "elf",
      "bin",
      "zip",
      "0",
    ]);
    if (file) await inspect(file);
  }

  async function pickFolder() {
    const folder = await pickers.pickDirectory("Choose a folder");
    if (folder) await inspect(folder);
  }

  async function run(kind: string, out: string) {
    if (!info) return;
    setBusy(true);
    try {
      await api.convertRun(kind, info.path, out, chunkMb);
      toast(`${kind} started`);
      reload();
    } catch (err) {
      toast(`${kind} failed: ${String(err)}`);
    } finally {
      setBusy(false);
    }
  }

  async function extractPkg() {
    if (!info) return;
    const out = await pickers.pickDirectory("Choose an output folder");
    if (out) await run("pkg_extract", out);
  }

  async function splitFile() {
    if (!info) return;
    await run("split", info.path);
  }

  async function mergeParts() {
    if (!info) return;
    const out = await pickers.pickSavePath("Save merged file", info.name.replace(/\.\d+$/, ""), ["pkg"]);
    if (out) await run("merge", out);
  }

  async function install() {
    if (!info) return;
    try {
      await api.installPath(info.path);
      toast(`sending ${info.name} to the console`);
      reload();
    } catch (err) {
      toast(`install failed: ${String(err)}`);
    }
  }

  return (
    <div className="page">
      <div className="panel">
        <div className="panel-head">
          <h2>Conversions</h2>
          <div className="spacer" />
          <Btn small onClick={() => void pickFile()}>
            Choose file
          </Btn>
          <Btn small onClick={() => void pickFolder()}>
            Choose folder
          </Btn>
        </div>

        {!info ? (
          <Empty
            title="Pick something to work on"
            hint="Unpack a PKG, split a package for a FAT32 USB drive, merge a split set, or send a PKG or payload to the console."
          />
        ) : (
          <>
            <div className="rows">
              <div className="row">
                <div>
                  <div className="title mono">{info.name}</div>
                  <div className="sub mono" title={info.path}>
                    {info.path}
                  </div>
                </div>
                <div className="grow" />
                <Chip tone="laser">{info.kind}</Chip>
                <Chip tone={info.system === "unknown" ? "warn" : "good"}>{info.system}</Chip>
                <span className="size mono">{fmtBytes(info.size_bytes)}</span>
              </div>
            </div>

            <div className="stats" style={{ marginTop: 12 }}>
              <Stat k="title id" v={info.title_id ?? "-"} />
              <Stat k="parts" v={info.parts ? String(info.parts) : "-"} />
              <Stat k="destination" v={destination(info)} />
            </div>

            <div className="quick" style={{ marginTop: 14 }}>
              {info.kind === "pkg" ? (
                <>
                  <Btn variant="primary" disabled={busy} onClick={() => void extractPkg()}>
                    Unpack to folder
                  </Btn>
                  <Btn disabled={busy} onClick={() => void splitFile()}>
                    Split for FAT32
                  </Btn>
                </>
              ) : null}
              {info.kind === "split" ? (
                <Btn variant="primary" disabled={busy} onClick={() => void mergeParts()}>
                  Merge parts
                </Btn>
              ) : null}
              {["pkg", "payload", "zip"].includes(info.kind) ? (
                <Btn
                  variant={info.kind === "pkg" ? "ghost" : "primary"}
                  disabled={busy}
                  onClick={() => void install()}
                >
                  Send to PS4
                </Btn>
              ) : null}
              {["pkg", "payload", "zip"].includes(info.kind) ? (
                <Btn disabled={busy} onClick={() => void splitFile()}>
                  Split for FAT32
                </Btn>
              ) : null}
            </div>

            <div className="field" style={{ marginTop: 14 }}>
              <label>Split chunk size (MiB)</label>
              <input
                type="number"
                value={chunkMb}
                onChange={(e) => setChunkMb(Number(e.target.value))}
              />
            </div>
          </>
        )}
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>What each tool does</h2>
        </div>
        <div className="checks">
          <Note
            name="Unpack a PKG"
            detail="Decrypts the package and writes the PFS tree out as a normal folder: eboot.bin, sce_sys, data."
          />
          <Note
            name="Split for FAT32"
            detail={`Writes <name>.0, <name>.1 ... at ${chunkMb} MiB for USB drives that cap a file at 4 GiB.`}
          />
          <Note name="Merge parts" detail="Joins a split set back into one PKG." />
          <Note
            name="Building a PKG"
            detail="Out of scope: a fake PKG carries Sony's package signature. Use a PKG builder, then install the result here."
          />
        </div>
      </div>
    </div>
  );
}

function destination(info: ConvertInfo): string {
  if (info.kind === "pkg") return "console installer (or /user/pkg)";
  if (info.kind === "payload") return "bin loader (port 9090)";
  return "-";
}

function Stat({ k, v }: { k: string; v: string }) {
  return (
    <div className="stat">
      <div className="v">{v}</div>
      <div className="k">{k}</div>
    </div>
  );
}

function Note({ name, detail }: { name: string; detail: string }) {
  return (
    <div className="check">
      <span className="status-dot idle" />
      <div>
        <div className="name">{name}</div>
        <div className="detail">{detail}</div>
      </div>
    </div>
  );
}
