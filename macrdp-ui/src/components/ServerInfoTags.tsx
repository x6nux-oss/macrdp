import { Monitor, Wrench, Maximize, Lock } from "lucide-react";
import { Badge } from "./ui/badge";

interface ServerInfoTagsProps {
  encoder?: string;
  chromaMode?: string;
  resolution?: string;
  tls?: boolean;
}

export default function ServerInfoTags({ encoder, chromaMode, resolution, tls }: ServerInfoTagsProps) {
  const codec = chromaMode === "avc444" ? "H.264 GFX · AVC444" : "H.264 GFX · AVC420";
  const encoderLabel = encoder === "hardware" ? "VideoToolbox" : encoder === "software" ? "OpenH264" : "Auto";

  return (
    <div className="flex flex-wrap gap-2 mb-5">
      <Badge variant="secondary" className="gap-1.5">
        <Monitor className="h-3 w-3" /> {codec}
      </Badge>
      <Badge variant="secondary" className="gap-1.5">
        <Wrench className="h-3 w-3" /> {encoderLabel}
      </Badge>
      {resolution && (
        <Badge variant="secondary" className="gap-1.5">
          <Maximize className="h-3 w-3" /> {resolution}
        </Badge>
      )}
      {tls && (
        <Badge variant="secondary" className="gap-1.5">
          <Lock className="h-3 w-3" /> TLS
        </Badge>
      )}
    </div>
  );
}
