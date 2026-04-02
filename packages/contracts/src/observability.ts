export interface KvReadonlySnapshot {
  digitCount: number;
  objects: KvReadonlyObjectEntry[];
}

export interface KvReadonlyObjectEntry {
  number: string;
  objectKey: string;
}
