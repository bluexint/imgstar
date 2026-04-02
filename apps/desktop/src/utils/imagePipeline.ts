import type { PluginConfig, UploadFileRef } from "@imgstar/contracts";
import type { ImageProcessingOptions } from "@/types/imageProcessing";

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, value));

const isImageLike = (file: File): boolean => {
  if (file.type) {
    return file.type.startsWith("image/");
  }

  return /\.(png|jpe?g|webp|gif|bmp|svg)$/i.test(file.name);
};

const isPluginEnabled = (pluginChain: PluginConfig[], id: string): boolean =>
  pluginChain.some((plugin) => plugin.enabled && plugin.id === id);

const sanitizeFileName = (name: string): string =>
  name.replace(/[^a-zA-Z0-9._-]/g, "_");

const toInlinePath = (name: string): string => `inline/${sanitizeFileName(name)}`;

const bytesToBase64 = (bytes: Uint8Array): string => {
  let binary = "";
  const chunkSize = 0x8000;

  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    const chunk = bytes.subarray(offset, offset + chunkSize);
    binary += String.fromCharCode(...chunk);
  }

  return btoa(binary);
};

const loadImageElement = async (file: File): Promise<HTMLImageElement> => {
  return await new Promise((resolve, reject) => {
    const image = new Image();
    const url = URL.createObjectURL(file);

    image.onload = () => {
      URL.revokeObjectURL(url);
      resolve(image);
    };

    image.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("image_decode_failed"));
    };

    image.src = url;
  });
};

const canvasToBlob = async (
  canvas: HTMLCanvasElement,
  mimeType: string,
  quality?: number
): Promise<Blob> => {
  return await new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => {
        if (blob) {
          resolve(blob);
          return;
        }
        reject(new Error("image_encode_failed"));
      },
      mimeType,
      quality
    );
  });
};

const resolveOutputType = (sourceType: string): string => {
  if (sourceType === "image/png") {
    return "image/png";
  }
  if (sourceType === "image/webp") {
    return "image/webp";
  }
  return "image/jpeg";
};

const renderImage = async (
  file: File,
  options: ImageProcessingOptions,
  enableCompress: boolean,
  enableWatermark: boolean
): Promise<File> => {
  if (typeof document === "undefined") {
    return file;
  }

  const image = await loadImageElement(file);
  const maxEdge = Math.max(320, Math.floor(options.maxEdge));

  let width = image.naturalWidth;
  let height = image.naturalHeight;
  const currentMax = Math.max(width, height);
  if (enableCompress) {
    if (currentMax > maxEdge) {
      const ratio = maxEdge / currentMax;
      width = Math.max(1, Math.round(width * ratio));
      height = Math.max(1, Math.round(height * ratio));
    }
  }

  // Lossless default: keep original bytes when no transform is required.
  if (enableCompress && !enableWatermark) {
    const noResizeNeeded = currentMax <= maxEdge;
    const noReencodeNeeded = options.quality >= 0.999;
    if (noResizeNeeded && noReencodeNeeded) {
      return file;
    }
  }

  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;

  const context = canvas.getContext("2d");
  if (!context) {
    return file;
  }

  context.drawImage(image, 0, 0, width, height);

  if (enableWatermark && options.watermarkText.trim().length > 0) {
    const mark = options.watermarkText.trim();
    const base = Math.max(width, height);
    const fontSize = clamp(Math.round(base * 0.018), 12, 28);

    context.save();
    context.globalAlpha = clamp(options.watermarkOpacity, 0.08, 0.8);
    context.fillStyle = "#f8fafc";
    context.textAlign = "right";
    context.textBaseline = "bottom";
    context.font = `${fontSize}px Segoe UI`;
    context.fillText(mark, width - 16, height - 12);
    context.restore();
  }

  const outputType = resolveOutputType(file.type);
  const quality = outputType === "image/png" ? undefined : clamp(options.quality, 0.4, 1);
  const blob = await canvasToBlob(canvas, outputType, quality);

  return new File([blob], file.name, {
    type: outputType,
    lastModified: file.lastModified
  });
};

export async function buildUploadFileRef(
  baseRef: UploadFileRef,
  localFile: File,
  activePlugins: PluginConfig[],
  imageOptions: ImageProcessingOptions
): Promise<UploadFileRef> {
  const enableCompress = isPluginEnabled(activePlugins, "image-compress");
  const enableWatermark = isPluginEnabled(activePlugins, "hidden-watermark");

  let preparedFile = localFile;
  if (isImageLike(localFile) && (enableCompress || enableWatermark)) {
    try {
      preparedFile = await renderImage(
        localFile,
        imageOptions,
        enableCompress,
        enableWatermark
      );
    } catch {
      preparedFile = localFile;
    }
  }

  const bytes = new Uint8Array(await preparedFile.arrayBuffer());

  return {
    ...baseRef,
    path: toInlinePath(preparedFile.name),
    name: preparedFile.name,
    size: preparedFile.size,
    mimeType: preparedFile.type || baseRef.mimeType,
    inlineContentBase64: bytesToBase64(bytes)
  };
}
