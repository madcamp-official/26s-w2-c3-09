import { Controller, ForbiddenException, Get, UseGuards } from '@nestjs/common';
import { CurrentPrincipal } from '../auth/auth-principal';
import type { AuthPrincipal } from '../auth/auth-principal';
import { FirebaseAuthGuard } from '../auth/firebase-auth.guard';
import { HomeService } from './home.service';

@Controller('v1/home')
@UseGuards(FirebaseAuthGuard)
export class HomeController {
  constructor(private readonly home: HomeService) {}

  @Get('summary')
  summary(@CurrentPrincipal() principal: AuthPrincipal) {
    if (principal.authType !== 'FIREBASE') {
      throw new ForbiddenException({
        code: 'FORBIDDEN',
        message: 'Firebase user token required',
      });
    }
    return this.home.summary(principal.userId);
  }
}
