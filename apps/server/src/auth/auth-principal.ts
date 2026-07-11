import { createParamDecorator, ExecutionContext } from '@nestjs/common';

export interface AuthPrincipal {
  userId: string;
  authProviderUid: string;
  displayName: string;
  authType: 'FIREBASE' | 'DEVICE';
  deviceId: string | null;
}
export const CurrentPrincipal = createParamDecorator(
  (_data: unknown, context: ExecutionContext): AuthPrincipal => {
    return context.switchToHttp().getRequest<{ principal: AuthPrincipal }>()
      .principal;
  },
);
