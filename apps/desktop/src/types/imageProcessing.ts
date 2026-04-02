export interface ImageProcessingOptions {
  maxEdge: number;
  quality: number;
  watermarkText: string;
  watermarkOpacity: number;
}

export const DEFAULT_IMAGE_OPTIONS: ImageProcessingOptions = {
  maxEdge: 16384,
  quality: 1,
  watermarkText: "imgstar",
  watermarkOpacity: 0.22
};
