import { Plus } from "lucide-react";
import { useStore } from "../store/useStore";
import { Button } from "./Button";

export function EmptyState({ message }: { message: string }) {
  const openCapture = useStore((s) => s.openCapture);
  return (
    <div className="empty">
      <p>{message}</p>
      <Button variant="primary" onClick={openCapture}>
        <Plus size={15} /> Capture
      </Button>
    </div>
  );
}
