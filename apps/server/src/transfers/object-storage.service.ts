import { Injectable, ServiceUnavailableException } from '@nestjs/common';
import {
  DeleteObjectCommand,
  GetObjectCommand,
  HeadObjectCommand,
  PutObjectCommand,
  S3Client,
} from '@aws-sdk/client-s3';
import { getSignedUrl } from '@aws-sdk/s3-request-presigner';
import { z } from 'zod';

const schema = z.object({
  OBJECT_STORAGE_ENDPOINT: z.url(),
  OBJECT_STORAGE_REGION: z.string().min(1),
  OBJECT_STORAGE_BUCKET: z.string().min(1),
  OBJECT_STORAGE_ACCESS_KEY_ID: z.string().min(1),
  OBJECT_STORAGE_SECRET_ACCESS_KEY: z.string().min(1),
});
@Injectable()
export class ObjectStorageService {
  private config() {
    const result = schema.safeParse(process.env);
    if (!result.success)
      throw new ServiceUnavailableException({
        code: 'UNCONFIGURED',
        provider: 'OBJECT_STORAGE',
      });
    return result.data;
  }
  private client() {
    const c = this.config();
    return {
      config: c,
      client: new S3Client({
        endpoint: c.OBJECT_STORAGE_ENDPOINT,
        region: c.OBJECT_STORAGE_REGION,
        forcePathStyle: true,
        credentials: {
          accessKeyId: c.OBJECT_STORAGE_ACCESS_KEY_ID,
          secretAccessKey: c.OBJECT_STORAGE_SECRET_ACCESS_KEY,
        },
      }),
    };
  }
  assertConfigured() {
    this.config();
  }
  async uploadUrl(key: string, expiresIn: number) {
    const { client, config } = this.client();
    return getSignedUrl(
      client,
      new PutObjectCommand({ Bucket: config.OBJECT_STORAGE_BUCKET, Key: key }),
      { expiresIn },
    );
  }
  async downloadUrl(key: string, expiresIn: number) {
    const { client, config } = this.client();
    return getSignedUrl(
      client,
      new GetObjectCommand({ Bucket: config.OBJECT_STORAGE_BUCKET, Key: key }),
      { expiresIn },
    );
  }
  async size(key: string) {
    const { client, config } = this.client();
    return (
      await client.send(
        new HeadObjectCommand({
          Bucket: config.OBJECT_STORAGE_BUCKET,
          Key: key,
        }),
      )
    ).ContentLength;
  }
  async delete(key: string) {
    const { client, config } = this.client();
    await client.send(
      new DeleteObjectCommand({
        Bucket: config.OBJECT_STORAGE_BUCKET,
        Key: key,
      }),
    );
  }
}
