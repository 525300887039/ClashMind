import * as Accordion from "@radix-ui/react-accordion";
import { ChevronDown } from "lucide-react";
import { useMemo } from "react";
import type { ProxiesResponse, ProxyGroup, ProxyNode } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";
import { ProxyNodeCard } from "./proxy-node-card";
import { DelayTestButton } from "./delay-test-button";
import {
  useSwitchProxy,
  useTestGroupDelay,
} from "./hooks/use-proxies";

function isProxyGroup(
  entry: ProxyNode | ProxyGroup,
): entry is ProxyGroup {
  return "all" in entry && Array.isArray((entry as ProxyGroup).all);
}

interface ParsedGroup {
  group: ProxyGroup;
  nodes: ProxyNode[];
}

function parseGroups(data: ProxiesResponse): ParsedGroup[] {
  const { proxies } = data;
  const result: ParsedGroup[] = [];

  for (const entry of Object.values(proxies)) {
    if (!isProxyGroup(entry)) continue;
    if (entry.name === "GLOBAL") continue;

    const nodes: ProxyNode[] = [];
    for (const nodeName of entry.all) {
      const node = proxies[nodeName];
      if (node && !isProxyGroup(node)) {
        nodes.push(node);
      }
    }
    result.push({ group: entry, nodes });
  }

  return result;
}

export function ProxyGroupList({ data }: { data: ProxiesResponse }) {
  const switchProxy = useSwitchProxy();
  const testGroupDelay = useTestGroupDelay();

  const groups = useMemo(() => parseGroups(data), [data]);

  return (
    <Accordion.Root type="multiple" className="space-y-4">
      {groups.map(({ group, nodes }) => (
        <Accordion.Item
          key={group.name}
          value={group.name}
          className="overflow-hidden rounded-[1.5rem] border border-border/70 bg-background/95 shadow-md"
        >
          <Accordion.Header className="flex">
            <Accordion.Trigger
              className={cn(
                "flex flex-1 items-center gap-2 px-5 py-4 text-base font-semibold",
                "hover:bg-muted/50 [&[data-state=open]>svg.chevron]:rotate-180",
              )}
            >
              <ChevronDown className="chevron size-4 shrink-0 text-muted-foreground transition-transform" />
              <span>{group.name}</span>
              <span className="rounded-full border border-primary/15 bg-primary/10 px-1.5 py-0.5 text-xs text-primary">
                {group.type}
              </span>
              <span className="ml-auto mr-2 truncate rounded-full bg-muted/40 px-3 py-1 text-xs font-medium text-foreground">
                {group.now}
              </span>
              <DelayTestButton
                loading={testGroupDelay.isPending}
                onClick={() => testGroupDelay.mutate({ group: group.name })}
              />
            </Accordion.Trigger>
          </Accordion.Header>
          <Accordion.Content className="overflow-hidden data-[state=closed]:animate-accordion-up data-[state=open]:animate-accordion-down">
            <div className="grid grid-cols-2 gap-3 px-5 pb-5 md:grid-cols-3 lg:grid-cols-4">
              {nodes.map((node) => (
                <ProxyNodeCard
                  key={node.name}
                  name={node.name}
                  type={node.type}
                  delay={node.delay}
                  selected={node.name === group.now}
                  onSelect={() =>
                    switchProxy.mutate({
                      group: group.name,
                      name: node.name,
                    })
                  }
                />
              ))}
            </div>
          </Accordion.Content>
        </Accordion.Item>
      ))}
    </Accordion.Root>
  );
}
