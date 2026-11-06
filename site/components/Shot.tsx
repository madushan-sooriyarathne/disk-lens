import styles from "./Shot.module.css";

type Name = "overview" | "explore" | "recommended" | "palette";

// Screenshots are crops of docs/mockup-v2.png (mock data); the dark crop is swapped in by color scheme.
export function Shot({ name, alt, priority = false }: { name: Name; alt: string; priority?: boolean }) {
  return (
    <picture className={styles.shot}>
      <source media="(prefers-color-scheme: dark)" srcSet={`/shot-${name}-dark.png`} />
      <img
        src={`/shot-${name}.png`}
        alt={alt}
        width={1290}
        height={830}
        loading={priority ? "eager" : "lazy"}
        fetchPriority={priority ? "high" : "auto"}
      />
    </picture>
  );
}
