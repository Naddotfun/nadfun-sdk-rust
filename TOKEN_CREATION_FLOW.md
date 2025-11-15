# Token Creation Flow Documentation

This document describes the complete flow for creating a token on the NAD platform. The process consists of three sequential API calls.

---

## Overview

The token creation process follows these steps:

1. **Upload Image** - Upload and validate the token image
2. **Upload Metadata** - Create and store token metadata
3. **Mine Salt** - Generate a vanity address salt for token deployment

```
┌─────────────────┐      ┌──────────────────┐      ┌─────────────────┐
│  Upload Image   │ ───> │ Upload Metadata  │ ───> │   Mine Salt     │
│ /metadata/image │      │ /metadata/metadata│      │  /token/salt    │
└─────────────────┘      └──────────────────┘      └─────────────────┘
       │                         │                          │
       ▼                         ▼                          ▼
  image_uri              metadata_uri                    salt
   is_nsfw                                              address
```

---

## Step 1: Upload Image

Upload the token image with automatic NSFW validation.

### Endpoint
```
POST /metadata/image
```

### Request

**Content-Type**: `image/png`, `image/jpeg`, `image/webp`, or `image/svg+xml`

**Body**: Raw binary image data

**Size Limit**: 5MB maximum

**Example (cURL)**:
```bash
curl -X POST https://api.nadapp.net/metadata/image \
  -H "Content-Type: image/png" \
  --data-binary @./my-token-image.png
```

**Example (JavaScript)**:
```javascript
const imageFile = document.querySelector('input[type="file"]').files[0];

const response = await fetch('https://api.nadapp.net/metadata/image', {
  method: 'POST',
  headers: {
    'Content-Type': imageFile.type
  },
  body: imageFile
});

const data = await response.json();
```

### Response

**Status Code**: `200 OK`

**Body**:
```json
{
  "image_uri": "https://storage.nadapp.net/image-94a412d2-b599-4bb0-b026-b14c4036c58c.png",
  "is_nsfw": false
}
```

**Response Fields**:
- `image_uri` (string): CDN URL of the uploaded image
- `is_nsfw` (boolean): NSFW classification result from AI validation

### Error Responses

| Status Code | Description |
|-------------|-------------|
| 400 | Invalid image format or missing image |
| 413 | Image exceeds 5MB limit |
| 500 | NSFW check failed or upload failed |

**Example Error**:
```json
{
  "error": "Invalid image format"
}
```

---

## Step 2: Upload Metadata

Create and store token metadata using the image URI from Step 1.

### Endpoint
```
POST /metadata/metadata
```

### Request

**Content-Type**: `application/json`

**Body**:
```json
{
  "image_uri": "https://storage.nadapp.net/image-94a412d2-b599-4bb0-b026-b14c4036c58c.png",
  "name": "My Token",
  "symbol": "MTK",
  "description": "An awesome token for the NAD community",
  "website": "https://mytoken.com",
  "twitter": "https://x.com/mytoken",
  "telegram": "https://t.me/mytoken"
}
```

**Required Fields**:
- `image_uri` (string): Image URI from Step 1 (must be from allowed domain)
- `name` (string): Token name (cannot be empty)
- `symbol` (string): Token symbol (cannot be empty)
- `description` (string): Token description (cannot be empty)

**Optional Fields**:
- `website` (string | null): Website URL (must start with `https://`)
- `twitter` (string | null): X (Twitter) URL (must contain `x.com` and start with `https://`)
- `telegram` (string | null): Telegram URL (must contain `t.me` and start with `https://`)

**Validation Rules**:
- All URLs must use HTTPS
- Twitter URLs must contain `x.com`
- Telegram URLs must contain `t.me`
- Image URI must be from the allowed domain (e.g., `https://storage.nadapp.net/`)

**Example (cURL)**:
```bash
curl -X POST https://api.nadapp.net/metadata/metadata \
  -H "Content-Type: application/json" \
  -d '{
    "image_uri": "https://storage.nadapp.net/image-94a412d2-b599-4bb0-b026-b14c4036c58c.png",
    "name": "My Token",
    "symbol": "MTK",
    "description": "An awesome token for the NAD community",
    "website": "https://mytoken.com",
    "twitter": "https://x.com/mytoken",
    "telegram": "https://t.me/mytoken"
  }'
```

**Example (JavaScript)**:
```javascript
const response = await fetch('https://api.nadapp.net/metadata/metadata', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    image_uri: imageUri, // from Step 1
    name: 'My Token',
    symbol: 'MTK',
    description: 'An awesome token for the NAD community',
    website: 'https://mytoken.com',
    twitter: 'https://x.com/mytoken',
    telegram: 'https://t.me/mytoken'
  })
});

const data = await response.json();
```

### Response

**Status Code**: `200 OK`

**Body**:
```json
{
  "metadata_uri": "https://storage.nadapp.net/metadata-94a412d2-b599-4bb0-b026-b14c4036c58c.json",
  "metadata": {
    "name": "My Token",
    "symbol": "MTK",
    "description": "An awesome token for the NAD community",
    "image_uri": "https://storage.nadapp.net/image-94a412d2-b599-4bb0-b026-b14c4036c58c.png",
    "website": "https://mytoken.com",
    "twitter": "https://x.com/mytoken",
    "telegram": "https://t.me/mytoken",
    "is_nsfw": false
  }
}
```

**Response Fields**:
- `metadata_uri` (string): CDN URL of the metadata JSON file
- `metadata` (object): The complete metadata object with NSFW flag

### Error Responses

| Status Code | Description |
|-------------|-------------|
| 400 | NSFW status unknown for image, invalid data, or validation failed |
| 500 | Upload to R2 or database failed |

**Example Error**:
```json
{
  "error": "Invalid image URI - must be from https://storage.nadapp.net/"
}
```

---

## Step 3: Mine Salt

Generate a salt value to create a vanity token address (address ending with specific digits).

### Endpoint
```
POST /token/salt
```

### Request

**Content-Type**: `application/json`

**Body**:
```json
{
  "creator": "0x742d35Cc6634C0532925a3b844Bc9e7595f70143",
  "name": "My Token",
  "symbol": "MTK",
  "metadata_uri": "https://storage.nadapp.net/metadata-94a412d2-b599-4bb0-b026-b14c4036c58c.json"
}
```

**Fields**:
- `creator` (string): Creator's wallet address (EVM format)
- `name` (string): Token name (must match metadata)
- `symbol` (string): Token symbol (must match metadata)
- `metadata_uri` (string): Metadata URI from Step 2

**Note**: The salt mining algorithm finds a salt that produces a token address ending with "777" (or other desired suffix).

**Example (cURL)**:
```bash
curl -X POST https://api.nadapp.net/token/salt \
  -H "Content-Type: application/json" \
  -d '{
    "creator": "0x742d35Cc6634C0532925a3b844Bc9e7595f70143",
    "name": "My Token",
    "symbol": "MTK",
    "metadata_uri": "https://storage.nadapp.net/metadata-94a412d2-b599-4bb0-b026-b14c4036c58c.json"
  }'
```

**Example (JavaScript)**:
```javascript
const response = await fetch('https://api.nadapp.net/token/salt', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    creator: walletAddress,
    name: 'My Token',
    symbol: 'MTK',
    metadata_uri: metadataUri // from Step 2
  })
});

const data = await response.json();
```

### Response

**Status Code**: `200 OK`

**Body**:
```json
{
  "salt": "0x000000000000000000000000000000000000000000000000000000000000a3f5",
  "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f70777"
}
```

**Response Fields**:
- `salt` (string): The mined salt value (32 bytes hex with 0x prefix)
- `address` (string): The resulting token address with desired suffix

### Error Responses

| Status Code | Description |
|-------------|-------------|
| 400 | Invalid parameters (e.g., invalid creator address) |
| 408 | Request timeout - max iterations reached without finding salt |
| 500 | Internal server error |

**Example Error**:
```json
{
  "error": "Max iterations reached",
  "iterations_attempted": 1000000
}
```

---

## Complete Flow Example

Here's a complete example in JavaScript:

```javascript
async function createToken(imageFile, tokenData, creatorAddress) {
  try {
    // Step 1: Upload Image
    console.log('Step 1: Uploading image...');
    const imageResponse = await fetch('https://api.nadapp.net/metadata/image', {
      method: 'POST',
      headers: {
        'Content-Type': imageFile.type
      },
      body: imageFile
    });

    if (!imageResponse.ok) {
      throw new Error('Image upload failed');
    }

    const { image_uri, is_nsfw } = await imageResponse.json();
    console.log('Image uploaded:', image_uri);
    console.log('NSFW:', is_nsfw);

    // Step 2: Upload Metadata
    console.log('Step 2: Uploading metadata...');
    const metadataResponse = await fetch('https://api.nadapp.net/metadata/metadata', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        image_uri,
        name: tokenData.name,
        symbol: tokenData.symbol,
        description: tokenData.description,
        website: tokenData.website,
        twitter: tokenData.twitter,
        telegram: tokenData.telegram
      })
    });

    if (!metadataResponse.ok) {
      throw new Error('Metadata upload failed');
    }

    const { metadata_uri } = await metadataResponse.json();
    console.log('Metadata uploaded:', metadata_uri);

    // Step 3: Mine Salt
    console.log('Step 3: Mining salt...');
    const saltResponse = await fetch('https://api.nadapp.net/token/salt', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        creator: creatorAddress,
        name: tokenData.name,
        symbol: tokenData.symbol,
        metadata_uri
      })
    });

    if (!saltResponse.ok) {
      throw new Error('Salt mining failed');
    }

    const { salt, address } = await saltResponse.json();
    console.log('Salt mined:', salt);
    console.log('Token address:', address);

    return {
      image_uri,
      is_nsfw,
      metadata_uri,
      salt,
      address
    };

  } catch (error) {
    console.error('Token creation failed:', error);
    throw error;
  }
}

// Usage
const imageFile = document.querySelector('input[type="file"]').files[0];
const tokenData = {
  name: 'My Token',
  symbol: 'MTK',
  description: 'An awesome token for the NAD community',
  website: 'https://mytoken.com',
  twitter: 'https://x.com/mytoken',
  telegram: 'https://t.me/mytoken'
};
const creatorAddress = '0x742d35Cc6634C0532925a3b844Bc9e7595f70143';

const result = await createToken(imageFile, tokenData, creatorAddress);
console.log('Token creation complete:', result);
```

---

## Important Notes

1. **Sequential Process**: Each step depends on the output of the previous step
   - Step 2 requires `image_uri` from Step 1
   - Step 3 requires `metadata_uri` from Step 2

2. **NSFW Validation**:
   - Images are automatically checked for NSFW content in Step 1
   - The NSFW flag is included in the final metadata

3. **URL Validation**:
   - All social media and website URLs must use HTTPS
   - Twitter URLs must use `x.com` domain
   - Telegram URLs must use `t.me` domain

4. **Image Domain Restriction**:
   - Only images from the allowed domain can be used in metadata
   - This ensures all images are properly stored and validated

5. **Salt Mining**:
   - The salt mining process generates vanity addresses
   - May take time depending on the desired suffix pattern
   - Has a timeout limit to prevent infinite loops

6. **Smart Contract Integration**:
   - Use the returned `salt` and `address` values when deploying the token contract
   - These values ensure the deployed token has the desired vanity address

---

## API Base URL

**Production**: `https://api.nadapp.net`
**Development**: Configure via `ENVIRONMENT` and `ALLOW_CORS_PORT` environment variables

---

## Rate Limiting

Rate limits may apply to prevent abuse. Contact the NAD team for rate limit details.

---

## Support

For issues or questions about the token creation flow:
- GitHub: https://github.com/anthropics/claude-code/issues
- Documentation: https://docs.nadapp.net
