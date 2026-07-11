import {
  CanActivate,
  ExecutionContext,
  ForbiddenException,
  Injectable,
  UnauthorizedException,
} from '@nestjs/common';
import { Reflector } from '@nestjs/core';
import { AGENT_ONLY } from './agent-only.decorator';
import { AuthService } from './auth.service';
import type { AuthPrincipal } from './auth-principal';

@Injectable()
export class FirebaseAuthGuard implements CanActivate {
  constructor(
    private readonly auth: AuthService,
    private readonly reflector: Reflector,
  ) {}
  async canActivate(context: ExecutionContext) {
    const request = context.switchToHttp().getRequest<{
      headers: { authorization?: string };
      principal?: AuthPrincipal;
    }>();
    const header = request.headers.authorization;
    if (!header?.startsWith('Bearer '))
      throw new UnauthorizedException({ code: 'UNAUTHENTICATED' });
    const token = header.slice(7);
    request.principal = token.startsWith('hm_device_')
      ? await this.auth.authenticateDevice(token.slice(10))
      : await this.auth.authenticate(token);
    const agentOnly = this.reflector.getAllAndOverride<boolean>(AGENT_ONLY, [
      context.getHandler(),
      context.getClass(),
    ]);
    if (agentOnly && request.principal.authType !== 'DEVICE')
      throw new ForbiddenException({
        code: 'FORBIDDEN',
        message: 'Active desktop device token required',
      });
    return true;
  }
}
