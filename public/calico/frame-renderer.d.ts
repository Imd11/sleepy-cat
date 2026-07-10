export function loadImageBitmapSurface(file: string, options?: Record<string, unknown>): Promise<any>;
export function loadHtmlImageSurface(file: string, options?: Record<string, unknown>): Promise<any>;
export function calculateContainRect(
  sourceWidth: number,
  sourceHeight: number,
  targetWidth: number,
  targetHeight: number
): { x: number; y: number; width: number; height: number };
export function frameGeometry(
  sheet: any,
  frameIndex: number,
  targetWidth: number,
  targetHeight: number
): any;
export function playbackFrameAt(sheet: any, elapsedMs: number): any;
export function createCalicoFrameRenderer(options: any): any;
