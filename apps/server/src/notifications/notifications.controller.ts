import {
  Body,
  Controller,
  Delete,
  Param,
  Post,
  UseGuards,
} from '@nestjs/common';
import { registerPushNotificationTokenSchema } from '@mousekeeper/contracts';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { ZodValidationPipe } from '../common/zod-validation.pipe';
import { NotificationsService } from './notifications.service';

@Controller('v1/notification-tokens')
@UseGuards(FirebaseAuthGuard)
export class NotificationsController {
  constructor(private readonly notifications: NotificationsService) {}

  @Post()
  register(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Body(new ZodValidationPipe(registerPushNotificationTokenSchema))
    body: { token: string; platform: 'ANDROID' | 'IOS' },
  ) {
    return this.notifications.register(principal.userId, body);
  }

  @Delete(':id')
  revoke(
    @CurrentPrincipal() principal: AuthPrincipal,
    @Param('id') id: string,
  ) {
    return this.notifications.revoke(principal.userId, id);
  }
}
