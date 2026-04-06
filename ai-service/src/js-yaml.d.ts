declare module "js-yaml" {
  export interface DumpOptions {
    lineWidth?: number;
    noArrayIndent?: boolean;
    noCompatMode?: boolean;
    noRefs?: boolean;
    sortKeys?: boolean;
  }

  export function load(input: string): unknown;
  export function dump(input: unknown, options?: DumpOptions): string;

  const yaml: {
    load: typeof load;
    dump: typeof dump;
  };

  export default yaml;
}
