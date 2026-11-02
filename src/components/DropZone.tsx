import { open } from "@tauri-apps/plugin-dialog";
import { FileText, Upload } from "lucide-react";
import { Chip } from "./Chip";

export const SUPPORTED = ["txt", "md", "vtt", "eml", "json", "csv", "docx", "pdf"];

function basename(p: string): string {
  return p.split(/[\\/]/).pop() ?? p;
}

export function DropZone({
  files,
  onAdd,
  onRemove,
  isOver,
}: {
  files: string[];
  onAdd: (paths: string[]) => void;
  onRemove: (path: string) => void;
  isOver: boolean;
}) {
  const browse = async () => {
    const picked = await open({
      multiple: true,
      directory: false,
      filters: [{ name: "Context", extensions: SUPPORTED }],
    });
    if (!picked) return;
    onAdd(Array.isArray(picked) ? picked : [picked]);
  };

  return (
    <div className={`dropzone ${isOver ? "over" : ""}`}>
      {files.length === 0 ? (
        <>
          <Upload size={15} />
          <span>
            Drop meeting transcripts, emails, notes&hellip; or{" "}
            <button className="browse" onClick={browse}>
              browse
            </button>
          </span>
        </>
      ) : (
        <>
          {files.map((f) => (
            <Chip key={f} title={f} onRemove={() => onRemove(f)}>
              <FileText size={11} style={{ marginRight: 4 }} />
              {basename(f)}
            </Chip>
          ))}
          <button className="browse" onClick={browse}>
            add more
          </button>
        </>
      )}
    </div>
  );
}
