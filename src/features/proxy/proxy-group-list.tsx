import * as Accordion from "@radix-ui/react-accordion";
import { ChevronDown } from "lucide-react";
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

  const groups = parseGroups(data);

  return (
    <Accordion.Root type="multiple" className="space-y-2">
      {groups.map(({ group, nodes }) => (
        <Accordion.Item
          key={group.name}
          value={group.name}
          className="rounded-lg border border-border"
        >
          <Accordion.Header className="flex">
            <Accordion.Trigger
              className={cn(
                "flex flex-1 items-center gap-2 px-4 py-3 text-sm font-medium",
                "hover:bg-muted/50 [&[data-state=open]>svg.chevron]:rotate-180",
              )}
            >
              <ChevronDown className="chevron size-4 shrink-0 text-muted-foreground transition-transform" />
              <span>{group.name}</span>
              <span className="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
                {group.type}
              </span>
              <span className="ml-auto mr-2 truncate text-xs text-muted-foreground">
                {group.now}
              </span>
              <DelayTestButton
                loading={testGroupDelay.isPending}
                onClick={() => testGroupDelay.mutate({ group: group.name })}
              />
            </Accordion.Trigger>
          </Accordion.Header>
          <Accordion.Content className="overflow-hidden data-[state=closed]:animate-accordion-up data-[state=open]:animate-accordion-down">
            <div className="grid grid-cols-2 gap-2 px-4 pb-4 md:grid-cols-3">
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
