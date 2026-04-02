const isImageLike = (file: File): boolean => {
  if (file.type) {
    return file.type.startsWith("image/");
  }

  return /\.(png|jpe?g|webp|gif|bmp|svg)$/i.test(file.name);
};

const resolveOutputMimeType = (file: File): string => {
  const type = file.type.toLowerCase();
  if (type === "image/png" || file.name.toLowerCase().endsWith(".png")) {
    return "image/png";
  }

  if (type === "image/webp" || file.name.toLowerCase().endsWith(".webp")) {
    return "image/webp";
  }

  return "image/jpeg";
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
      reject(new Error("preview_image_decode_failed"));
    };

    image.src = url;
  });
};

const canvasToDataUrl = (
  canvas: HTMLCanvasElement,
  mimeType: string,
  quality?: number
): string => canvas.toDataURL(mimeType, quality);

export const fileToDataUrl = async (file: File): Promise<string> => {
  return await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === "string") {
        resolve(reader.result);
        return;
      }

      reject(new Error("preview_data_url_failed"));
    };
    reader.onerror = () => reject(new Error("preview_data_url_failed"));
    reader.readAsDataURL(file);
  });
};

export const createThumbnailDataUrl = async (
  file: File,
  maxEdge = 240
): Promise<string | undefined> => {
  if (!isImageLike(file) || typeof document === "undefined") {
    return undefined;
  }

  try {
    const image = await loadImageElement(file);
    const longestEdge = Math.max(image.naturalWidth, image.naturalHeight);
    const scale = longestEdge > maxEdge ? maxEdge / longestEdge : 1;
    const width = Math.max(1, Math.round(image.naturalWidth * scale));
    const height = Math.max(1, Math.round(image.naturalHeight * scale));
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;

    const context = canvas.getContext("2d");
    if (!context) {
      return undefined;
    }

    context.drawImage(image, 0, 0, width, height);

    const mimeType = resolveOutputMimeType(file);
    const quality = mimeType === "image/png" ? undefined : 0.82;
    return canvasToDataUrl(canvas, mimeType, quality);
  } catch {
    try {
      return await fileToDataUrl(file);
    } catch {
      return undefined;
    }
  }
};