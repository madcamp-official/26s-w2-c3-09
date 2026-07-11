import { SetMetadata } from '@nestjs/common';
export const AGENT_ONLY = 'housemouse.agentOnly';
export const AgentOnly = () => SetMetadata(AGENT_ONLY, true);
